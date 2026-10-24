/* Independent scene controls and rendering of server-computed sight polygons. */
(() => {
 const oldPanel=renderPanel;
 renderPanel=function(){oldPanel();if(!state)return;
  if(tab==='settings')$('#panel-body').prepend(button('Scène : murs, portes et vision',openSceneRules));
  if(tab==='scenes'&&gm())$('#panel-body').prepend(button('Murs, portes et vision',openSceneRules));
 };
 function openSceneRules(){
  let d=$('#advanced-scene');if(d)d.remove();d=moduleWindow('advanced-scene','Scène — Portes et vision','advanced-window');const body=d.querySelector('.module-window-body'),s=scene();if(!s)return;
  body.append(el('p','module-intro','Vision : portée de chaque pion partagé ou possédé, arrêtée par les murs. La carte reste une image téléchargée ; ce masque visuel ne protège pas ses détails. Les déplacements du MJ ignorent les collisions.'));
  if(gm()){
   const form=el('form');for(const [key,label]of [['collision','Bloquer les déplacements des joueurs à travers les murs'],['vision_enabled','Limiter la vision des joueurs aux pions qu’ils contrôlent']]){const l=el('label','advanced-check',label),c=el('input');c.type='checkbox';c.name=key;c.checked=!!s[key];l.prepend(c);form.append(l)}const save=el('button','module-primary','Appliquer les règles de scène');save.type='submit';form.append(save);form.onsubmit=async e=>{e.preventDefault();if(await action('scene_rules',{collision:form.elements.collision.checked,vision_enabled:form.elements.vision_enabled.checked}))d.close()};body.append(form);
   body.append(el('h3','','Vision des pions (cases de grille)'));for(const t of s.tokens){const row=el('div','advanced-row'),label=el('label','',t.name),input=el('input');input.type='number';input.min=0;input.max=60;input.step=1;input.value=t.vision_cells??6;label.append(input);row.append(label,button('Enregistrer',async()=>{if(await action('token_vision',{id:t.id,vision_cells:Number(input.value)}))toast('Portée enregistrée')}));body.append(row)}
  }
  body.append(el('h3','','Murs et portes'),el('p','','Tracez un segment avec l’outil Mur, puis transformez-le en porte ici. Double-cliquez sur une porte de la carte pour l’ouvrir ou la fermer. Un joueur doit être à moins de 1,6 case ; une porte verrouillée exige le MJ.'));
  s.walls.forEach((wall,index)=>{const row=el('section','module-card');row.append(el('strong','',`${wall.door?'Porte':'Mur'} ${index+1} — ${wall.door?({open:'ouverte',closed:'fermée',locked:'verrouillée'}[wall.state]||'fermée'):''}`));
   if(wall.door)row.append(button(wall.state==='open'?'Fermer':'Ouvrir',async()=>{if(await action('door_toggle',{index,wall})){d.close();openSceneRules()}}));
   if(gm())row.append(button('Configurer',()=>dialog('Configurer le segment '+(index+1),[{name:'door',label:'Porte',type:'checkbox',value:wall.door},{name:'locked',label:'Verrouillée',type:'checkbox',value:wall.state==='locked'},{name:'open',label:'Ouverte',type:'checkbox',value:wall.state==='open'},{name:'blocks_move',label:'Bloque le mouvement',type:'checkbox',value:wall.blocks_move!==false},{name:'blocks_sight',label:'Bloque la vision',type:'checkbox',value:wall.blocks_sight!==false}],async v=>{const ok=await action('wall_update',{index,wall,...v,state:v.locked?'locked':v.open?'open':'closed'});if(ok){d.close();openSceneRules()}return ok})),button('Supprimer',async()=>{if(await action('wall_delete',{index,wall})){d.close();openSceneRules()}}));body.append(row)});
  if(!s.walls.length)body.append(el('p','','Aucun mur : utilisez le calque Murs puis tracez un segment sur la carte.'));d.show();
 }
 function inside(p,poly){let result=false;for(let i=0,j=poly.length-1;i<poly.length;j=i++){const a=poly[i],b=poly[j];if(((a[1]>p.y)!==(b[1]>p.y))&&(p.x<(b[0]-a[0])*(p.y-a[1])/(b[1]-a[1])+a[0]))result=!result}return result}
 function doorVisible(w,s){if(gm()||!s.vision_enabled)return true;const p={x:(w.x1+w.x2)/2,y:(w.y1+w.y2)/2};return (s.vision_polygons||[]).some(poly=>inside(p,poly))||s.tokens.some(t=>(t.owner===''||t.owner===state.user.name)&&Math.hypot(p.x-t.x,p.y-t.y)<=s.grid*1.6)}
 const mask=document.createElement('canvas'),oldDraw=draw;
 draw=function(){oldDraw();const s=scene();if(!s||!state)return;
  if(s.vision_enabled&&!gm()){
   if(mask.width!==canvas.width||mask.height!==canvas.height){mask.width=canvas.width;mask.height=canvas.height}const m=mask.getContext('2d');m.setTransform(1,0,0,1,0,0);m.globalCompositeOperation='source-over';m.fillStyle='#080e10';m.fillRect(0,0,mask.width,mask.height);
   m.setTransform(devicePixelRatio,0,0,devicePixelRatio,0,0);m.translate(offset.x,offset.y);m.scale(scale,scale);m.globalCompositeOperation='destination-out';for(const poly of s.vision_polygons||[]){if(!poly.length)continue;m.beginPath();m.moveTo(...poly[0]);for(const p of poly.slice(1))m.lineTo(...p);m.closePath();m.fill()}m.globalCompositeOperation='source-over';ctx.save();ctx.setTransform(1,0,0,1,0,0);ctx.drawImage(mask,0,0);ctx.restore();
  }
  ctx.save();ctx.setTransform(devicePixelRatio,0,0,devicePixelRatio,0,0);ctx.translate(offset.x,offset.y);ctx.scale(scale,scale);
  for(const wall of s.walls||[]){if(!wall.door||!doorVisible(wall,s))continue;const x=(wall.x1+wall.x2)/2,y=(wall.y1+wall.y2)/2;ctx.fillStyle=wall.state==='open'?'#3e8d61':wall.state==='locked'?'#ac403b':'#c39448';ctx.strokeStyle='#eee3cb';ctx.lineWidth=1.5/scale;ctx.fillRect(x-9/scale,y-9/scale,18/scale,18/scale);ctx.strokeRect(x-9/scale,y-9/scale,18/scale,18/scale);ctx.fillStyle='#fff';ctx.font=`bold ${12/scale}px sans-serif`;ctx.textAlign='center';ctx.textBaseline='middle';ctx.fillText(wall.state==='open'?'○':wall.state==='locked'?'×':'▯',x,y)}ctx.restore();
 };
 const previousDouble=canvas.ondblclick;canvas.ondblclick=function(e){const s=scene(),p=point(e);if(s&&tool==='select'){const index=s.walls.findIndex(w=>w.door&&doorVisible(w,s)&&Math.hypot(p.x-(w.x1+w.x2)/2,p.y-(w.y1+w.y2)/2)<16/scale);if(index>=0){action('door_toggle',{index,wall:s.walls[index]});return}}previousDouble(e)};
 if(state)render();
})();
