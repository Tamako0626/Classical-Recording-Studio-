import {llmConfig,llmRecognize} from './llm-settings.js';

const supported='图片、PDF、DOCX、TXT、CUE、CSV、TSV、MD、JSON、XML、SRT 或 VTT';
const extension=/\.(png|jpe?g|webp|bmp|gif|heic|heif|pdf|docx|txt|cue|csv|tsv|md|json|xml|srt|vtt)$/iu;
const images=/\.(png|jpe?g|webp|bmp|gif|heic|heif)$/iu;
const accept='.png,.jpg,.jpeg,.webp,.bmp,.gif,.heic,.heif,.pdf,.docx,.txt,.cue,.csv,.tsv,.md,.json,.xml,.srt,.vtt';
const maxText=50000;

async function jpeg(file){
  const bitmap=await createImageBitmap(file);
  try{
    const canvas=document.createElement('canvas');
    const scale=Math.min(1,2300/Math.max(bitmap.height,bitmap.width));
    canvas.width=Math.max(1,Math.round(bitmap.width*scale));
    canvas.height=Math.max(1,Math.round(bitmap.height*scale));
    const ctx=canvas.getContext('2d',{alpha:false});
    ctx.fillStyle='#fff';
    ctx.fillRect(0,0,canvas.width,canvas.height);
    ctx.drawImage(bitmap,0,0,canvas.width,canvas.height);
    const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/jpeg',0.86));
    if(!blob)throw Error('无法准备图片。');
    return new Uint8Array(await blob.arrayBuffer());
  }finally{bitmap.close();}
}
async function readDocx(file){
  const {unzipSync,strFromU8}=await import('/llm-zip.js');
  const contents=unzipSync(new Uint8Array(await file.arrayBuffer()),{filter:entry=>
    entry.name==='word/document.xml'&&entry.originalSize<=6000000});
  if(!contents['word/document.xml'])throw Error('DOCX 缺少正文，或解压后的正文过大。');
  const xml=new DOMParser().parseFromString(strFromU8(contents['word/document.xml']),'application/xml');
  if(xml.querySelector('parsererror'))throw Error('DOCX 正文格式无效。');
  const ns='http://schemas.openxmlformats.org/wordprocessingml/2006/main';
  return [...xml.getElementsByTagNameNS(ns,'p')].map(p=>[...p.getElementsByTagNameNS(ns,'t')].map(t=>t.textContent).join('')).join('\n');
}
async function readPdf(file,addImage,report,signal){
  const pdf=await import('/llm-pdf.mjs');
  pdf.GlobalWorkerOptions.workerSrc='/llm-pdf-worker.mjs';
  const task=pdf.getDocument({data:new Uint8Array(await file.arrayBuffer()),isEvalSupported:false,
    cMapUrl:location.origin+'/llm-cmaps/',cMapPacked:true,standardFontDataUrl:location.origin+'/llm-fonts/',useWasm:false});
  const cancel=()=>{void task.destroy().catch(()=>{});};
  signal.addEventListener('abort',cancel,{once:true});
  try{
    const document=await task.promise;
    if(document.numPages>20)throw Error('PDF 每次最多读取 20 页，请先拆分文档。');
    for(let number=1;number<=document.numPages;number++){
      signal.throwIfAborted();
      report('正在读取 PDF 第 '+number+' / '+document.numPages+' 页…');
      const page=await document.getPage(number);
      const items=(await page.getTextContent()).items;
      const words=items.filter(item=>item.str).map(item=>item.str+(item.hasEOL?'\n':' ')).join('').trim();
      if(words.length>=32){await addImage({text:words,name:file.name+' / '+number});continue;}
      const viewport=page.getViewport({scale:1});
      const scaled=page.getViewport({scale:Math.min(3,2100/Math.max(viewport.width,viewport.height))});
      const canvas=globalThis.document.createElement('canvas');
      canvas.width=Math.ceil(scaled.width);canvas.height=Math.ceil(scaled.height);
      await page.render({canvas,viewport:scaled,background:'#fff'}).promise;
      const blob=await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));
      if(!blob)throw Error('无法读取 PDF 扫描页。');
      await addImage({file:blob,name:file.name+' / '+number});
    }
  }finally{
    signal.removeEventListener('abort',cancel);
    if(!signal.aborted)await task.destroy();
  }
}
async function readText(file){
  const bytes=await file.arrayBuffer();
  if(bytes.byteLength>512000)throw Error('文本文件超过 512 KB。');
  if(/\.cue$/iu.test(file.name)){
    const {decodeCue}=await import('./cd-import-data.js');
    return decodeCue(bytes);
  }
  const prefix=new Uint8Array(bytes,0,Math.min(2,bytes.byteLength));
  if(prefix[0]===255&&prefix[1]===254)return new TextDecoder('utf-16le').decode(bytes);
  if(prefix[0]===254&&prefix[1]===255)return new TextDecoder('utf-16be').decode(bytes);
  return new TextDecoder('utf-8',{fatal:true}).decode(bytes);
}
export async function recognizeFiles({files,text,task,report,signal,config}){
  if(!config.model||config.provider==='deepseek'&&!config.hasKey)throw Error('请先在右上角「LLM 配置」保存模型和 API Key。');
  const sources=[text.trim()].filter(Boolean),pictures=[],ocr=[];
  let textLength=sources[0]?.length||0;
  function append(name,content,index=sources.length){
    if(!content.trim())return;
    const part='['+name+']\n'+content.trim();
    textLength+=part.length+2;
    if(textLength>maxText)throw Error('待识别文字超过 50,000 字符，请拆分输入。');
    sources[index]=part;
  }
  async function prepare({file,text,name}){
    signal.throwIfAborted();
    if(text){append(name,text);return;}
    if(!config.vision){ocr.push({file,name,text:'',error:'',index:sources.push('')-1});return;}
    if(pictures.length>=8)throw Error('每次最多向视觉模型发送 8 张图片或扫描页。');
    pictures.push(await jpeg(file));
  }
  if(files.length>20)throw Error('每次最多选择 20 个文件。');
  for(const file of files){
    signal.throwIfAborted();
    if(!extension.test(file.name))throw Error('支持的资料为 '+supported+'。');
    if(!file.size||file.size>20000000)throw Error(file.name+' 为空或超过 20 MB。');
    report('正在准备 '+file.name+'…');
    if(images.test(file.name)){
      let image=file;
      if(/\.hei[cf]$/iu.test(file.name)){
        const {value,error}=await window.studioNative.convertCdArtwork(new Uint8Array(await file.arrayBuffer()));
        if(error)throw Error(error);
        image=new Blob([value],{type:'image/png'});
      }
      await prepare({file:image,name:file.name});
    }
    if(/\.pdf$/iu.test(file.name))await readPdf(file,prepare,report,signal);
    if(/\.docx$/iu.test(file.name))append(file.name,await readDocx(file));
    if(!images.test(file.name)&&!/\.(pdf|docx)$/iu.test(file.name))append(file.name,await readText(file));
  }
  if(ocr.length){
    if(ocr.length>20)throw Error('每次最多本地 OCR 20 张图片或扫描页。');
    const {recognizePages}=await import('./program-import.js');
    await recognizePages(ocr,(page,progress)=>report(page?'本地 OCR · '+page.name+(progress==null?'…':' · '+progress+'%'):'正在加载中英文 OCR…'),signal);
    if(ocr.some(page=>!page.text))throw Error('本地 OCR 未能读取 '+ocr.filter(page=>!page.text).map(page=>page.name).join('、')+'。可选择视觉模型或补充文字后重试。');
    for(const page of ocr)append(page.name,page.text,page.index);
  }
  signal.throwIfAborted();
  if(!sources.length&&!pictures.length)throw Error('资料中没有可识别的内容。');
  report('正在使用 '+config.model+' 识别…');
  return llmRecognize(task,sources.join('\n\n'),pictures,config);
}

export async function openLlmFiles({task='general',text='',files=[],apply}={}){
  const dialog=document.createElement('dialog');
  dialog.className='program-dialog';
  dialog.setAttribute('aria-label','模型识别文件');
  dialog.innerHTML=`<div class="dialog-head"><h2>模型识别文件</h2><button class="button quiet" data-close>关闭</button></div>
    <div class="dialog-body">
      <label class="field"><span>选择资料，可多选</span><input type="file" data-files multiple accept="${accept}"></label>
      <div class="hint" data-existing></div>
      <label class="field section-gap"><span>补充文字</span><textarea rows="5" maxlength="50000" data-text></textarea></label>
      <p class="hint" data-info>选择文件或粘贴文字。支持 ${supported}。</p>
      <label data-disclosure><input type="checkbox" data-consent> <span data-destination>正在读取模型配置…</span></label>
      <p class="hint" role="status" data-status></p>
      <label class="field section-gap" data-result-field hidden><span>识别结果，可直接校对</span><textarea rows="12" maxlength="50000" data-result></textarea></label>
    </div>
    <div class="dialog-foot"><button type="button" class="button primary" data-run>开始识别</button><button type="button" class="button" data-apply hidden>使用校对结果</button></div>`;
  const $=selector=>dialog.querySelector(selector);
  const controller=new AbortController();
  const run=$('[data-run]'),use=$('[data-apply]'),picker=$('[data-files]'),source=$('[data-text]'),result=$('[data-result]');
  const consent=$('[data-consent]'),destination=$('[data-destination]');
  let activeConfig;
  const endpointId=config=>[config.provider,config.base,config.model,config.vision].join('|');
  async function readConfig(){
    const config=await llmConfig();
    if(!config.model||config.provider==='deepseek'&&!config.hasKey)throw Error('请先在右上角「LLM 配置」保存模型和 API Key。');
    const material=config.vision?'所选图片或扫描页及提取的文字':'输入文字、提取的正文和本机 OCR 文字';
    destination.textContent='同意将'+material+'发送到 '+new URL(config.base).host+'（'+config.model+'）。';
    return config;
  }
  source.value=text;
  $('[data-existing]').textContent=files.length?'已带入 '+files.map(file=>file.name).join('、')+'。':'';
  const say=message=>{$('[data-status]').textContent=message;};
  $('[data-close]').onclick=()=>dialog.close();
  dialog.addEventListener('close',()=>{controller.abort();dialog.remove();},{once:true});
  use.onclick=()=>{if(!result.value.trim()){say('请先填写要使用的识别结果。');return;}apply(result.value);dialog.close();};
  run.onclick=async()=>{
    const selected=[...new Set([...files,...picker.files])];
    const input=source.value;
    run.disabled=use.disabled=picker.disabled=source.disabled=true;
    try{
      const config=await readConfig();
      if(!activeConfig||endpointId(config)!==endpointId(activeConfig))consent.checked=false;
      activeConfig=config;
      if(!selected.length&&!input.trim())throw Error('请选择文件或输入文字。');
      if(!consent.checked)throw Error('请确认同意将所选资料发送给模型服务。');
      consent.checked=false;
      const answer=await recognizeFiles({files:selected,text:input,task,report:say,signal:controller.signal,config});
      if(controller.signal.aborted)return;
      result.value=answer;
      $('[data-result-field]').hidden=false;
      use.hidden=!apply;
      say('识别完成，请核对曲目、日期及标签。');
      result.focus();
    }catch(error){if(!controller.signal.aborted)say(error.message);}
    finally{run.disabled=use.disabled=picker.disabled=source.disabled=false;}
  };
  document.body.append(dialog);
  dialog.showModal();
  consent.disabled=true;
  try{activeConfig=await readConfig();}
  catch(error){destination.textContent=error.message;}
  finally{consent.disabled=false;}
}
