import{spawn}from'node:child_process';import{mkdtemp,readFile}from'node:fs/promises';import{tmpdir}from'node:os';import{join,resolve}from'node:path';import assert from'node:assert/strict';
const dir=await mkdtemp(join(tmpdir(),'rusty-advanced-')),port=3031,url=`http://127.0.0.1:${port}`,binary=resolve(process.argv[2]||'target/debug/rusty-vtt.exe');let server;
async function boot(){server=spawn(binary,[],{env:{...process.env,RUSTY_BIND:`127.0.0.1:${port}`,RUSTY_DATA:join(dir,'world.json'),RUSTY_GM_PASSWORD:'test'},stdio:'pipe',windowsHide:true});let log='';server.stderr.on('data',d=>log+=d);server.on('error',e=>log+=e.message);for(let n=0;n<100;n++){try{if((await fetch(url,{signal:AbortSignal.timeout(500)})).ok)return}catch{}await new Promise(r=>setTimeout(r,100))}throw Error('Serveur indisponible : '+log)}
async function stop(){if(server&&!server.killed)await new Promise(r=>{server.once('exit',r);server.kill()})}
async function api(path,body,cookie,status=200){const r=await fetch(url+path,{method:body===undefined?'GET':'POST',headers:{'Content-Type':'application/json',...(cookie?{Cookie:cookie}:{})},body:body===undefined?undefined:JSON.stringify(body)});const raw=await r.text();assert.equal(r.status,status,path+' : '+raw);return{data:status===200?JSON.parse(raw):raw,cookie:r.headers.get('set-cookie')?.split(';')[0]}}
try{await boot();const gm=(await api('/api/login',{name:'MJ',password:'test'})).cookie,alice=(await api('/api/login',{name:'Alice'})).cookie,bob=(await api('/api/login',{name:'Bob'})).cookie,eve=(await api('/api/login',{name:'Eve'})).cookie;const act=(body,c=gm,status=200)=>api('/api/action',body,c,status);const state=async c=>(await api('/api/state',undefined,c)).data.world;
for(const mode of ['public','gm','blind','self','whisper'])await act({kind:'roll',formula:'1d6+10',mode,recipients:['Bob']},alice);
assert.deepEqual((await state(alice)).messages.map(m=>m.mode),['public','gm','self','whisper']);assert.deepEqual((await state(bob)).messages.map(m=>m.mode),['public','whisper']);assert.deepEqual((await state(eve)).messages.map(m=>m.mode),['public']);assert.deepEqual((await state(gm)).messages.map(m=>m.mode),['public','gm','blind']);
await act({kind:'chat',text:'bonjour privé',mode:'whisper',recipients:['Bob']},alice);assert.equal((await state(bob)).messages.at(-1).text,'bonjour privé');assert.ok(!(await state(eve)).messages.some(m=>m.text==='bonjour privé'));await act({kind:'chat',text:'interdit',mode:'blind'},alice,400);
await act({kind:'update_token',id:'hero',owner:'Alice'});await act({kind:'token_vision',id:'hero',vision_cells:20});
await act({kind:'scene_rules',collision:true,vision_enabled:true},alice,403);await act({kind:'scene_rules',collision:true,vision_enabled:true});
await act({kind:'wall_add',x1:500,y1:0,x2:500,y2:1000,door:true});
let wall=(await state(gm)).scenes[0].walls[0];assert.equal((await state(alice)).scenes[0].tokens.length,1);assert.equal((await state(bob)).scenes[0].tokens.length,0);
assert.ok((await state(alice)).scenes[0].vision_polygons[0].length>100);
await act({kind:'move_token',id:'hero',x:650,y:400},alice,400);
await act({kind:'door_toggle',index:0,wall},alice,400); // midpoint too far from hero (100,100 away)
await act({kind:'move_token',id:'hero',x:450,y:500},alice);await act({kind:'door_toggle',index:0,wall},alice);
assert.equal((await state(alice)).scenes[0].tokens.length,2);await act({kind:'move_token',id:'hero',x:650,y:500},alice);
await act({kind:'wall_update',index:0,wall,door:true,state:'locked',blocks_move:true,blocks_sight:true});await act({kind:'door_toggle',index:0,wall},alice,403);
await act({kind:'move_token',id:'hero',x:400,y:500},alice,400);await act({kind:'move_token',id:'hero',x:450,y:500},gm); // GM bypass
await act({kind:'update_actor',id:'hero',owner:'Alice'});
await act({kind:'effect_add',id:'hero',name:'À terre',description:'Désavantage manuel',rounds:2},alice);
await act({kind:'effect_add',id:'hero',name:'Interdit',rounds:2},bob,403);await act({kind:'effect_add',id:'hero',name:'Invalide',rounds:0},alice,400);
await act({kind:'combat_add',id:'hero'});await act({kind:'combat_next'});assert.equal((await state(gm)).actors[0].effects[0].rounds,1);await act({kind:'combat_next'});assert.equal((await state(gm)).actors[0].effects.length,0);
await act({kind:'effect_add',id:'hero',name:'Marqué',rounds:null},alice);const effect=(await state(alice)).actors[0].effects[0];await act({kind:'effect_toggle',id:'hero',effect_id:effect.id},alice);await act({kind:'combat_next'});assert.equal((await state(alice)).actors[0].effects[0].disabled,true);
await act({kind:'wall_add',x1:300,y1:0,x2:300,y2:1000});await act({kind:'wall_delete',index:0,wall});await act({kind:'door_toggle',index:0,wall},gm,409); // stale index guard
await api('/api/export',undefined,alice,403);const disk=JSON.parse(await readFile(join(dir,'world.json'),'utf8'));assert.equal(disk.messages.filter(m=>m.mode==='blind').length,1);
await stop();await boot();const cookie=(await api('/api/login',{name:'MJ',password:'test'})).cookie;const restored=await state(cookie);assert.equal(restored.scenes[0].collision,true);assert.equal(restored.actors[0].effects[0].disabled,true);
console.log('PASS — avancé : confidentialité des jets/murmures, collisions, vision, portes et verrous, droits, états temporisés, indices obsolètes et persistance.');
}finally{await stop()}
