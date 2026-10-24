// Regression coverage for QA-01..04; isolated world only.
import {spawn} from 'node:child_process';
import {mkdtemp,readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import assert from 'node:assert/strict';
const dir=await mkdtemp(join(tmpdir(),'rusty-qa-'));
const binary=resolve(process.argv[2]||'target/debug/rusty-vtt.exe');
const port=process.env.TEST_PORT||'3057',url=`http://127.0.0.1:${port}`;
const file=join(dir,'world.json');
let server,stderr='';
async function api(path,body,cookie,expected=200){
 const r=await fetch(url+path,{method:body===undefined?'GET':'POST',headers:{'Content-Type':'application/json',...(cookie?{Cookie:cookie}:{})},body:body===undefined?undefined:JSON.stringify(body),signal:AbortSignal.timeout(3000)});
 const raw=await r.text();assert.equal(r.status,expected,`${path}: ${raw}`);
 return {data:expected===200?JSON.parse(raw):raw,cookie:r.headers.get('set-cookie')?.split(';')[0]};
}
async function boot(){
 server=spawn(binary,[],{windowsHide:true,stdio:'pipe',env:{...process.env,RUSTY_BIND:`127.0.0.1:${port}`,RUSTY_DATA:file,RUSTY_GM_PASSWORD:'audit-gm-secret-123456',RUSTY_HTTPS:'0',RUSTY_PLAYERS_JSON:JSON.stringify({Alice:'alice-secret-123456',Bob:'bob-secret-12345678',Eve:'eve-secret-12345678'})}});
 let error;server.on('error',e=>error=e);server.stderr.on('data',d=>stderr+=d);
 for(let i=0;i<100;i++){
  if(error)throw error;
  if(server.exitCode!==null)throw Error(`Server exited: ${stderr}`);
  try{if((await fetch(url,{signal:AbortSignal.timeout(500)})).ok)return}catch{}
  await new Promise(r=>setTimeout(r,100));
 }
 throw Error(`Server unavailable: ${stderr}`);
}
async function stop(){if(server&&server.exitCode===null&&!server.killed)await new Promise(r=>{server.once('exit',r);server.kill()})}
try{
 await boot();
 let gm=(await api('/api/login',{name:'MJ',password:'audit-gm-secret-123456'})).cookie;
 const player=(await api('/api/login',{name:'Alice',password:'alice-secret-123456'})).cookie;
 const act=(body,cookie=gm,status=200)=>api('/api/action',body,cookie,status);
 await act({kind:'chat',text:'QA baseline'});
 const state=async cookie=>(await api('/api/state',undefined,cookie)).data;
 async function rejected(path,body,cookie,status){
  const before=await state(gm),disk=await readFile(file,'utf8');
  await api(path,body,cookie,status);
  assert.deepEqual(await state(gm),before,'Rejected request changed state or revision');
  assert.equal(await readFile(file,'utf8'),disk,'Rejected request changed save');
 }
 for(const fields of [{name:''},{name:'   '},{name:42},{name:'x'.repeat(101)},{owner:42},{color:[]},{hidden:'true'}]){
  await rejected('/api/action',{kind:'update_token',id:'hero',...fields},gm,400);
 }
 await act({kind:'update_token',id:'hero',name:'QA Hero',owner:'Alice',color:'#123456',hidden:false,hp:17});
 for(const value of [-1,0,1.5,10001,'24',null]){
  await rejected('/api/action',{kind:'update_actor',id:'hero',max_hp:value},player,400);
 }
 await act({kind:'update_actor',id:'hero',max_hp:1},player);
 await act({kind:'update_actor',id:'hero',max_hp:10000},player);
 assert.equal((await state(gm)).world.actors[0].max_hp,10000);
 const baseline=(await api('/api/export',undefined,gm)).data;
 for(const value of [42,'bad',[],null]){
  const malformed=structuredClone(baseline);malformed.actors[0].owner='MJ';malformed.actors[0].dnd5e=value;
  await rejected('/api/import',malformed,gm,400);
  await state(player);
 }
 const invalidHp=structuredClone(baseline);invalidHp.actors[0].max_hp=0;
 await rejected('/api/import',invalidHp,gm,400);
 for(const owner of ['', 'Alice']){
  await act({kind:'update_token',id:'hero',owner,hidden:true});
  await rejected('/api/action',{kind:'update_token',id:'hero',hp:1},player,403);
  await act({kind:'update_token',id:'hero',hp:16});
 }
 await act({kind:'update_token',id:'hero',hidden:false});
 await act({kind:'update_token',id:'hero',hp:15},player);
 const saved=JSON.parse(await readFile(file,'utf8'));
 await api('/api/import',saved,gm);
 await stop();await boot();gm=(await api('/api/login',{name:'MJ',password:'audit-gm-secret-123456'})).cookie;
 assert.deepEqual((await state(gm)).world,saved);
 assert.doesNotMatch(stderr,/panicked/);
 console.log('PASS — QA-01..04: validation, refus atomiques, import sans panic, pions cachés, bornes PV, redémarrage.');
}finally{await stop()}
