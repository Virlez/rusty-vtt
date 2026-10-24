labels.compendiums=['','Compendiums'];
function moduleWindow(id,title,cls){
 const d=el('dialog',cls);d.id=id;const head=el('div','window-title'),h=el('h2','',title),close=button('×',()=>d.close());close.title='Fermer';close.setAttribute('aria-label','Fermer '+title);head.append(h,close);const body=el('div','module-window-body');d.append(head,body);document.body.append(d);
 let drag=null;head.onpointerdown=e=>{if(e.target.closest('button'))return;const r=d.getBoundingClientRect();drag={x:e.clientX-r.left,y:e.clientY-r.top};head.setPointerCapture(e.pointerId)};head.onpointermove=e=>{if(!drag)return;d.style.transform='none';d.style.left=Math.max(0,Math.min(innerWidth-100,e.clientX-drag.x))+'px';d.style.top=Math.max(0,Math.min(innerHeight-35,e.clientY-drag.y))+'px'};head.onpointerup=()=>drag=null;
 return d;
}

let moduleDialog;
async function openModuleManager(){
 if(!gm())return;
 if(!moduleDialog)moduleDialog=moduleWindow('module-manager','Gestion des modules','module-manager');
 await fillModuleManager();if(!moduleDialog.open)moduleDialog.show();
}
async function fillModuleManager(){
 const box=moduleDialog.querySelector('.module-window-body');box.replaceChildren();
 for(const m of await request('/api/modules')){
  const card=el('section','module-card'),row=el('label','module-toggle'),check=el('input');check.type='checkbox';check.checked=m.enabled;
  row.append(check,el('strong','',m.title));card.append(row,el('p','',m.description),el('p','',m.adapter));
  check.onchange=async()=>{check.disabled=true;await action('module_toggle',{id:m.id,enabled:check.checked});await fillModuleManager()};
  if(m.enabled)card.append(button('Ouvrir le créateur',()=>{moduleDialog.close();openCharacterBuilder()}));box.append(card);
 }
}
function fillCompendiumPanel(){const box=$('#panel-body');box.replaceChildren(el('p','','Aucun compendium installé.'));$('#panel-add').style.display='none'}

const renderBeforeModules=renderPanel;
renderPanel=function(){renderBeforeModules();if(tab==='settings'&&gm())$('#panel-body').prepend(button('Gérer les modules',openModuleManager));if(tab==='compendiums')fillCompendiumPanel();};
