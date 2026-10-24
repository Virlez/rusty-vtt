/* Native descriptive status effects; server validates ownership and duration. */
function openActorEffects(id){
 const actor=state.world.actors.find(a=>a.id===id);if(!actor)return;
 const editable=gm()||actor.owner===''||actor.owner===state.user.name;
 $('#actor-effects')?.remove();
 const d=moduleWindow('actor-effects','États — '+actor.name,'module-manager'),body=d.querySelector('.module-window-body');
 body.append(el('p','module-intro','États descriptifs : aucun modificateur automatique. La durée baisse à chaque nouveau round pour les participants au combat. Un état suspendu conserve sa durée.'));
 for(const effect of actor.effects||[]){
  const row=el('div','card');row.append(el('h3','',effect.name),el('div','meta',(effect.disabled?'Suspendu · ':'')+(effect.rounds==null?'Durée libre':effect.rounds+' round(s) restant(s)')));
  if(effect.description)row.append(el('p','',effect.description));
  if(editable){const run=async kind=>{if(await action(kind,{id,effect_id:effect.id}))openActorEffects(id)};row.append(button(effect.disabled?'Reprendre':'Suspendre',()=>run('effect_toggle')),button('Retirer',()=>run('effect_remove')))}
  body.append(row);
 }
 if(!actor.effects?.length)body.append(el('p','','Aucun état actif.'));
 if(editable)body.append(button('Ajouter un état',()=>dialog('Nouvel état — '+actor.name,[{name:'name',label:'État (ex. À terre, Aveuglé)',required:true},{name:'rounds',label:'Durée en rounds (vide : libre)',type:'number'},{name:'description',label:'Conséquences à appliquer manuellement',type:'textarea'}],async v=>{
  const raw=$('#field-rounds').value.trim(),rounds=raw===''?null:Number(raw);
  if(rounds!==null&&(!Number.isInteger(rounds)||rounds<1||rounds>1000)){toast('Durée : 1 à 1000 rounds, ou champ vide.');return false}
  const ok=await action('effect_add',{id,name:v.name,description:v.description||'',rounds});if(ok)openActorEffects(id);return ok;
 })));
 d.show();
}
const effectsPreviousRenderPanel=renderPanel;
renderPanel=function(){
 effectsPreviousRenderPanel();if(!state?.world||!['actors','combat'].includes(tab))return;
 const body=$('#panel-body'),card=el('div','card');card.append(el('h3','','États et durées'));
 const ids=tab==='combat'?state.world.combat.order.map(e=>typeof e==='string'?e:e.id):null;
 for(const actor of state.world.actors.filter(a=>!ids||ids.includes(a.id))){
  const line=el('div'),active=(actor.effects||[]).filter(e=>!e.disabled);
  line.append(button(actor.name+' · '+active.length+' état(s)',()=>openActorEffects(actor.id)));
  if(active.length)line.append(el('p','meta',active.map(e=>e.name+(e.rounds==null?'':' ('+e.rounds+' r.)')).join(' · ')));
  card.append(line);
 }
 body.append(card);
};
