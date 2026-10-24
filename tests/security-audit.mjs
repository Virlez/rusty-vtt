import {spawn} from 'node:child_process';
import {mkdtemp} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import assert from 'node:assert/strict';
const dir=await mkdtemp(join(tmpdir(),'rusty-security-'));
const url='http://127.0.0.1:3029',binary=resolve('target/debug/rusty-vtt.exe');
const gmSecret='audit-gm-secret-123456';
const players={Alice:'alice-secret-123456',Eve:'eve-secret-12345678'};
let server,output='',proxy=false;
async function boot(extra={}){
 output='';proxy=extra.RUSTY_HTTPS==='1';
 const env={...process.env,RUSTY_BIND:'127.0.0.1:3029',RUSTY_DATA:join(dir,'world.json'),RUSTY_GM_PASSWORD:gmSecret,RUSTY_PLAYERS_JSON:JSON.stringify(players),RUSTY_HTTPS:'0',...extra};
 if(env.RUSTY_GM_PASSWORD===undefined)delete env.RUSTY_GM_PASSWORD;
 server=spawn(binary,[],{env,stdio:'pipe',windowsHide:true});
 let error;server.on('error',e=>error=e);
 server.stdout.on('data',d=>output+=d);server.stderr.on('data',d=>output+=d);
 for(let n=0;n<100;n++){
  if(error)throw error;
  if(server.exitCode!==null)throw Error('Audit server exited: '+output);
  try{if((await fetch(url,{signal:AbortSignal.timeout(500)})).ok)return;}catch{}
  await new Promise(r=>setTimeout(r,100));
 }
 throw Error('Audit server unavailable: '+output);
}
async function stop(){if(server?.exitCode===null)await new Promise(r=>{server.once('exit',r);server.kill();});}
async function api(path,body,cookie,status=200){
 const r=await fetch(url+path,{method:body===undefined?'GET':'POST',headers:{'Content-Type':'application/json',...(proxy?{'X-Forwarded-Proto':'https'}:{}),...(cookie?{Cookie:cookie}:{})},body:body===undefined?undefined:JSON.stringify(body),signal:AbortSignal.timeout(5000)});
 assert.equal(r.status,status,path+': HTTP '+r.status+' expected '+status);return r;
}
async function login(name,password=players[name]){
 const r=await api('/api/login',{name,password});
 return {cookie:r.headers.get('set-cookie').split(';')[0],header:r.headers.get('set-cookie'),state:await r.json()};
}
try{
 await boot();
 await api('/api/state',undefined,undefined,401);
 for(const [name,password] of [['MJ','rusty-gm'],['Alice',''],['Alice',players.Eve],['Unknown',players.Alice]])
  await api('/api/login',{name,password},undefined,401);
 const gm=await login('MJ',gmSecret),alice=await login('Alice'),eve=await login('Eve');
 assert.equal(alice.state.user.role,'player');
 const act=(v,cookie=gm.cookie,status=200)=>api('/api/action',v,cookie,status);
 await act({kind:'add_actor',name:'Private actor',owner:'Alice',notes:'AUDIT_PRIVATE_NOTES'});
 await act({kind:'chat',text:'AUDIT_PRIVATE_MESSAGE',mode:'self'},alice.cookie);
 const view=async(cookie)=>(await(await api('/api/state',undefined,cookie)).json()).world;
 const privateView=await view(eve.cookie),actorId=privateView.actors.at(-1).id;
 assert.equal(privateView.actors.at(-1).notes,'');
 assert.equal(privateView.messages.some(m=>m.text==='AUDIT_PRIVATE_MESSAGE'),false);
 await act({kind:'update_actor',id:actorId,hp:1},eve.cookie,403);
 await act({kind:'update_actor',id:actorId,hp:1},alice.cookie);
 await api('/api/export',undefined,eve.cookie,403);
 assert.equal(JSON.stringify(await view(gm.cookie)).includes(players.Alice),false);
 console.log('PASS SEC-01/02: configured secrets required; no default GM access; owner controls preserved');
 await act({kind:'combat_add',id:'wolf'});await act({kind:'combat_add',id:'hero'});
 await act({kind:'update_token',id:'wolf',hidden:true});
 let projected=await view(eve.cookie);
 assert.equal(projected.scenes[0].tokens.some(t=>t.id==='wolf'),false);
 assert.equal(projected.combat.order.some(t=>t.id==='wolf'),false);
 assert.ok(projected.combat.turn<projected.combat.order.length);
 assert.equal((await view(gm.cookie)).combat.order.some(t=>t.id==='wolf'),true);
 await act({kind:'update_token',id:'wolf',hidden:false});
 await act({kind:'fog_add',x:700,y:300,w:100,h:100});
 assert.equal((await view(eve.cookie)).combat.order.some(t=>t.id==='wolf'),false);
 await act({kind:'fog_clear'});
 console.log('PASS SEC-03: hidden/fogged combatants removed; GM view retained');
 assert.match(alice.header,/Max-Age=28800/);assert.match(alice.header,/HttpOnly/);assert.match(alice.header,/SameSite=Strict/);
 const stream=await fetch(url+'/api/events',{headers:{Cookie:alice.cookie},signal:AbortSignal.timeout(5000)});
 const reader=stream.body.getReader();
 await api('/api/logout',{},alice.cookie);await api('/api/state',undefined,alice.cookie,401);
 assert.equal((await reader.read()).done,true,'SSE must end after revocation');
 const rotated=await api('/api/login',{name:'Eve',password:players.Eve},eve.cookie);
 const rotatedCookie=rotated.headers.get('set-cookie').split(';')[0];
 await api('/api/state',undefined,eve.cookie,401);await api('/api/state',undefined,rotatedCookie);
 for(let n=1;n<8;n++)await login('Eve');
 await api('/api/login',{name:'Eve',password:players.Eve},undefined,429);
 let limited=false;
 for(let n=0;n<31;n++){
  const r=await fetch(url+'/api/login',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name:'MJ',password:'wrong'})});
  assert.ok([401,429].includes(r.status));if(r.status===429){limited=true;break;}
 }
 assert.equal(limited,true);
 console.log('PASS SEC-04: logout/rotation, session cap and login throttle');
 await stop();await boot();await api('/api/state',undefined,gm.cookie,401);
 const fresh=await login('Alice');
 for(let n=0;n<120;n++)await act({kind:'chat',text:'quota fixture'},fresh.cookie);
 await act({kind:'chat',text:'over quota'},fresh.cookie,429);
 console.log('PASS SEC-04: 120 mutations/minute/session limit');
 await stop();await boot({RUSTY_HTTPS:'1'});
 assert.equal((await fetch(url+'/api/state')).status,426);
 const secure=await login('Alice');assert.match(secure.header,/; Secure/);
 const logout=await api('/api/logout',{},secure.cookie);assert.match(logout.headers.get('set-cookie'),/Max-Age=0; Secure/);
 console.log('PASS SEC-05: Secure cookie in HTTPS proxy mode');
 await stop();await boot({RUSTY_GM_PASSWORD:undefined});
 const generated=output.match(/Secret MJ généré pour cette exécution : ([0-9a-f]{32})/);
 assert.ok(generated,'Missing random startup secret');
 await api('/api/login',{name:'MJ',password:'rusty-gm'},undefined,401);
 assert.equal((await login('MJ',generated[1])).state.user.role,'gm');
 console.log('PASS SEC-02: random GM secret when configuration is absent');
}finally{await stop();}
