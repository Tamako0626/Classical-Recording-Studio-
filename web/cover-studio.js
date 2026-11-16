import {artworkTypes,prepareArtwork} from './artwork.js';
import {llmConfig,llmRecognize} from './llm-settings.js';
import {openLlmFiles} from './llm-files.js';
const size=1600;
const serif='"Palatino Linotype", "Songti SC", "SimSun", Georgia, serif';
const layouts={
  ivory:{color:'#211b17',paper:'#f4eee2',x:.5,titleY:1255,photo:[0,0,size,1130],align:'center',ink:'light'},
  dark:{color:'#fff8ee',paper:'#24211f',x:.5,titleY:1255,photo:[0,0,size,1130],align:'center',ink:'light'},
  photo:{color:'#fffaf1',x:.075,titleY:1200,photo:[0,0,size,size],align:'left',ink:'shadow'}
};

function drawPhoto(ctx,image,[x,y,w,h],zoom,horizontal,vertical){
  const scale=Math.max(w/image.width,h/image.height)*zoom;
  const cropWidth=w/scale,cropHeight=h/scale;
  ctx.drawImage(image,(image.width-cropWidth)*horizontal,(image.height-cropHeight)*vertical,cropWidth,cropHeight,x,y,w,h);
}

function setFont(ctx,height,style=''){
  ctx.font=(style?style+' ':'')+height+'px '+serif;
}

function wrapText(ctx,text,width){
  const lines=[];
  for(const paragraph of text.split(/\r?\n/u)){
    let line='';
    const words=paragraph.trim().match(/\p{Script=Han}|[^\s\p{Script=Han}]+|\s+/gu)||[];
    for(const word of words){
      if(line.trim()&&ctx.measureText(line+word).width>width){
        lines.push(line.trim());line='';
      }
      if(ctx.measureText(word).width>width){
        for(const letter of word){
          if(line&&ctx.measureText(line+letter).width>width){lines.push(line.trim());line='';}
          line+=letter;
        }
      }else line+=word;
    }
    if(line.trim())lines.push(line.trim());
  }
  return lines;
}

function drawText(ctx,text,x,y,width,height,maxLines,style=''){
  if(!text)return y;
  let lines;
  do{
    setFont(ctx,height,style);
    lines=wrapText(ctx,text,width);
    if(lines.length<=maxLines)break;
    height-=2;
  }while(height>=22);
  const step=height*1.15;
  for(const [i,line] of lines.entries())ctx.fillText(line,x,y+step*i,width);
  return y+lines.length*step;
}

export function openCoverStudio({session,works=[],apply,download,convertArtwork}){
  const dialog=document.createElement('dialog');
  dialog.className='cover-studio';
  dialog.setAttribute('aria-label','从场照制作专辑封面');
  dialog.innerHTML='<div class="dialog-head"><h2>从场照制作专辑封面</h2><button type="button" class="button quiet" data-cover-action="close">关闭</button></div>'+
    '<div class="dialog-body cover-editor"><div class="cover-art"><canvas width="1600" height="1600" aria-label="专辑封面预览" hidden></canvas><p data-photo-prompt>上传场照后预览封面</p></div>'+
    '<div class="cover-fields"><label class="field"><span>场照 · PNG / JPG / WebP'+(convertArtwork?' / HEIC / HEIF':'')+'</span><input type="file" name="photo" accept="'+artworkTypes(convertArtwork)+',.webp"></label>'+
    '<div class="actions"><button type="button" class="button" data-cover-action="design" disabled>AI 设计封面</button><button type="button" class="button quiet" data-cover-action="undo" hidden>撤销 AI 方案</button></div>'+
    (window.studioNative?.llmGet?'<div class="actions"><button type="button" class="button quiet" data-cover-action="recognize">识别场照文字</button></div>':'')+
    '<p class="hint" data-ai-status role="status">正在检查模型连接…</p>'+
    '<label class="field"><span>排版</span><select name="layout"><option value="ivory">象牙白底 · 衬线标题</option><option value="dark">深色底 · 浅色衬线标题</option><option value="photo">通幅场照 · 左下角文字</option></select></label>'+
    '<label class="field"><span>专辑标题</span><textarea name="title" rows="2" maxlength="120"></textarea></label>'+
    '<label class="field"><span>演出者</span><input name="performer" maxlength="120"></label>'+
    '<label class="field"><span>音乐厅、城市与日期</span><input name="detail" maxlength="150"></label>'+
    '<div class="cover-sliders"><label class="field"><span>缩放</span><input type="range" name="zoom" min="1" max="3" step=".01" value="1"></label>'+
    '<label class="field"><span>水平裁剪</span><input type="range" name="horizontal" min="0" max="1" step=".01" value=".5"></label>'+
    '<label class="field"><span>垂直裁剪</span><input type="range" name="vertical" min="0" max="1" step=".01" value=".5"></label></div>'+
    '<p class="hint">1600 × 1600 PNG。场照仅裁剪和缩放，文字可直接编辑。</p><p class="form-error" role="alert"></p></div></div>'+
    '<div class="dialog-foot"><button class="button" type="button" data-cover-action="download" disabled>下载 PNG</button><button class="button primary" type="button" data-cover-action="apply" disabled>用作专辑封面</button></div>';
  const field=name=>dialog.querySelector('[name="'+name+'"]');
  const canvas=dialog.querySelector('canvas');
  const ctx=canvas.getContext('2d');
  const error=dialog.querySelector('.form-error');
  const actions=[...dialog.querySelectorAll('.dialog-foot button')];
  const aiStatus=dialog.querySelector('[data-ai-status]');
  const design=dialog.querySelector('[data-cover-action="design"]');
  const undo=dialog.querySelector('[data-cover-action="undo"]');
  const aiRequest=new AbortController();
  const aiFields=['title','performer','detail','layout'];
  const currentDesign=()=>Object.fromEntries(aiFields.map(name=>[name,field(name).value]));
  let previousDesign,coverConfig;
  let image, busy=false, pending=false, photoVersion=0;
  field('title').value=(session.name||session.composer||'').slice(0,120);
  field('performer').value=[...new Set([session.performer,session.conductor,session.orchestra].filter(Boolean))].join(' · ').slice(0,120);
  field('detail').value=[...new Set([session.venue,session.city,session.date].filter(Boolean))].join(' · ').slice(0,150);
  function refreshActions(){
    for(const button of actions)button.disabled=busy||pending||!image;
  }
  function redraw(){
    if(!image)return;
    const layout=layouts[field('layout').value];
    ctx.clearRect(0,0,size,size);
    if(layout.paper){ctx.fillStyle=layout.paper;ctx.fillRect(0,0,size,size);}
    drawPhoto(ctx,image,layout.photo,...['zoom','horizontal','vertical'].map(name=>Number(field(name).value)));
    ctx.save();
    ctx.fillStyle=layout.color;
    ctx.textAlign=layout.align;
    ctx.textBaseline='alphabetic';
    if(layout.ink==='shadow'){ctx.shadowColor='#15120e';ctx.shadowBlur=13;ctx.shadowOffsetY=2;}
    const x=size*layout.x,width=size*.85;
    let y=drawText(ctx,field('title').value.trim(),x,layout.titleY,width,77,2,'600');
    y=drawText(ctx,field('performer').value.trim(),x,y+5,width,42,2,'italic');
    drawText(ctx,field('detail').value.trim(),x,y+5,width,28,2);
    ctx.restore();
    canvas.hidden=false;
    dialog.querySelector('[data-photo-prompt]').hidden=true;
  }
  function applyDesign(values){
    for(const name of aiFields)field(name).value=values[name];
    redraw();
  }
  async function generateDesign(){
    if(design.disabled)return;
    design.disabled=true;
    const before=currentDesign();
    aiStatus.textContent='正在根据场次和曲目设计封面…';
    try{
      let result;
      if(window.studioNative?.llmGet){
        if(!coverConfig)throw Error('请先配置模型。');
        const resultText=await llmRecognize('cover',JSON.stringify({session,works,layouts:Object.keys(layouts)}),[],coverConfig);
        result=JSON.parse(resultText.replace(/^```(?:json)?\s*|\s*```$/gu,''));
        if(!Object.hasOwn(layouts,result?.layout)||aiFields.some(name=>typeof result[name]!=='string'))throw Error('模型没有返回有效的封面方案，请重试。');
        result={...result,title:result.title.slice(0,120),performer:result.performer.slice(0,120),detail:result.detail.slice(0,150)};
      }else{
        const response=await fetch('/api/cover-design',{
          method:'POST',headers:{'Content-Type':'application/json'},
          body:JSON.stringify({session,works}),signal:aiRequest.signal
        });
        result=await response.json();
        if(!response.ok)throw Error(result.error);
      }
      if(!dialog.open)return;
      if(aiFields.some(name=>field(name).value!==before[name])){
        aiStatus.textContent='生成期间已编辑封面。当前内容已保留，可重新生成。';
        return;
      }
      previousDesign=before;
      applyDesign(result);
      undo.hidden=false;
      aiStatus.textContent='AI 方案已填入，可继续编辑文字和排版。';
    }catch(err){
      if(!aiRequest.signal.aborted)aiStatus.textContent=err.message||'封面方案生成失败。';
    }finally{design.disabled=false;}
  }
  dialog.addEventListener('input',event=>{if(event.target!==field('photo'))redraw();});
  field('photo').onchange=async()=>{
    const file=field('photo').files[0];
    if(!file)return;
    const version=++photoVersion;
    pending=true;refreshActions();error.textContent='';
    try{
      if(!ctx)throw Error('无法准备封面画布。');
      const bitmap=await createImageBitmap(await prepareArtwork(file,convertArtwork,{webp:true,limit:30*1024*1024}));
      if(version!==photoVersion){bitmap.close();return;}
      image?.close();image=bitmap;
      for(const name of ['zoom','horizontal','vertical'])field(name).value=name==='zoom'?'1':'.5';
      redraw();
    }catch(err){if(version===photoVersion)error.textContent=err.message||'无法读取场照。';}
    finally{if(version===photoVersion){pending=false;refreshActions();}}
  };
  dialog.addEventListener('click',async event=>{
    const action=event.target.closest('[data-cover-action]')?.dataset.coverAction;
    if(action==='close'){dialog.close();return;}
    if(action==='recognize'){
      const file=field('photo').files[0];
      if(!file){aiStatus.textContent='请先上传场照。';return;}
      void openLlmFiles({files:[file]});
      return;
    }
    if(action==='design'){await generateDesign();return;}
    if(action==='undo'){applyDesign(previousDesign);undo.hidden=true;aiStatus.textContent='已恢复生成前的文案和排版。';return;}
    if(!action||busy||pending||!image)return;
    busy=true;refreshActions();error.textContent='';
    try{
      const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));
      if(!dialog.open)return;
      if(!blob)throw Error('PNG 导出失败，请重试。');
      const file=new File([blob],'cover.png',{type:'image/png'});
      if(action==='download')download(file);
      if(action==='apply'){apply(file);dialog.close();}
    }catch(err){error.textContent=err.message;}
    finally{busy=false;refreshActions();}
  });
  const focus=document.activeElement;
  dialog.addEventListener('close',()=>{aiRequest.abort();photoVersion++;image?.close();dialog.remove();focus?.focus();},{once:true});
  document.body.append(dialog);
  dialog.showModal();
  if(window.studioNative?.llmGet){
    llmConfig().then(config=>{
      const ready=config.model&&(config.provider!=='deepseek'||config.hasKey);
      design.disabled=!ready;
      coverConfig=ready?config:null;
      design.textContent='发送场次信息并设计封面';
      aiStatus.textContent=ready?'点击后会把场次文字和曲目发送到 '+new URL(config.base).host+'。场照在本机裁剪与缩放。':'可通过右上角「LLM 配置」连接模型。';
    }).catch(()=>{aiStatus.textContent='无法读取模型配置。';});
    return;
  }
  fetch('/api/cover-design',{signal:aiRequest.signal}).then(async response=>{
    if(!response.ok)throw Error();
    const {available}=await response.json();
    design.disabled=!available;
    aiStatus.textContent=available?'仅发送场次文字及曲目给 OpenAI。场照在本机参与排版。':'本站尚未连接 OpenAI API。可手动编辑封面。';
  }).catch(()=>{
    if(!aiRequest.signal.aborted)aiStatus.textContent='暂时无法检查模型连接。可手动编辑封面。';
  });
}
