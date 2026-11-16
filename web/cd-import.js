import {artworkTypes,prepareArtwork} from './artwork.js';
import {openLlmFiles} from './llm-files.js';
import {recognizePages} from './program-import.js';
import {cdFilename,decodeCue,parseCue,readCdText,sortCdFiles} from './cd-import-data.js';
import {outputFilenameValid} from './script.js';

const escape=value=>String(value??'').replace(/[&<>"']/gu,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const edit=(label,key,value='')=>'<label class="field"><span>'+label+'</span><input name="'+key+'" maxlength="280" value="'+escape(value)+'"></label>';
const button=(label,action,style='')=>'<button class="button '+style+'" type="button" data-cd="'+action+'">'+label+'</button>';
const two=n=>String(n).padStart(2,'0');

export function openCdImport({chooseFiles,convertArtwork,existingTracks,apply}){
  const dialog=document.createElement('dialog');
  dialog.className='program-dialog cd-dialog';
  dialog.setAttribute('aria-label','CD 抓轨整理');
  dialog.innerHTML='<div class="dialog-head"><h2>CD 抓轨整理</h2>'+button('关闭','close','quiet')+'</div>'+
    '<div class="dialog-body"><div class="form-grid">'+
    '<label class="field"><span>抓轨类型</span><select name="mode"><option value="cue">整轨 + CUE</option><option value="files">多个独立音轨</option></select></label>'+
    '<label class="field" data-cue-field><span>CUE 文件</span><input type="file" name="cue" accept=".cue"></label>'+
    '<div class="wide">'+button('选择抓轨音频','audio')+'<ol class="program-pages" data-cd-files></ol></div>'+
    '<label class="field"><span>封面（HEIC、HEIF、JPG、PNG）</span><input type="file" name="front" accept="'+artworkTypes(convertArtwork)+'"></label>'+
    '<label class="field"><span>封底（HEIC、HEIF、JPG、PNG）</span><input type="file" name="back" accept="'+artworkTypes(convertArtwork)+'"></label>'+
    '<label class="field wide"><span>曲目文字或图片识别原文</span><textarea name="text" rows="5" maxlength="50000" placeholder="1. 曲名&#10;2. 曲名"></textarea></label></div>'+
    '<div class="actions section-gap">'+button('识别封面与封底','ocr')+(window.studioNative?.llmGet?button('用模型识别封面与封底','llm'):'')+button('匹配文字到音轨','match')+'</div>'+
    '<p class="hint" data-cd-status role="status">选择音频，或先准备节目单文字。</p>'+
    '<section class="section-gap"><h3>专辑标签</h3><div class="form-grid">'+edit('专辑名称','album-title')+edit('专辑演出者','album-artist')+edit('作曲家','album-composer')+edit('日期或年份','album-date')+'</div></section>'+
    '<section class="section-gap"><h3>核对轨号、文件顺序和标签</h3><div class="table-wrap program-table" data-cd-tracks></div></section>'+
    (existingTracks?'<p><label><input type="checkbox" name="replace"> 导入时替换分轨页现有曲目</label></p>':'')+'</div>'+
    '<div class="dialog-foot">'+button('导入分轨页','apply','primary')+'</div>';
  const $=selector=>dialog.querySelector(selector);
  const input=name=>$('[name="'+name+'"]');
  const status=$('[data-cd-status]');
  const filesBox=$('[data-cd-files]'),tracksBox=$('[data-cd-tracks]');
  let files=[],rows=[],cueText='',cueNotice='',busy=false,controller=null;
  const artwork=new Map();
  const albumFields=['title','artist','composer','date'];
  const editedAlbum=new Set();
  let modelTracks;
  for(const key of albumFields)input('album-'+key).addEventListener('input',()=>editedAlbum.add(key));
  input('text').addEventListener('input',()=>{modelTracks=undefined;});
  const say=note=>{status.textContent=note;};
  function setBusy(value){
    busy=value;
    for(const el of dialog.querySelectorAll('input,select,textarea,button'))el.disabled=value&&el.dataset.cd!=='close';
  }
  function fillAlbum(album,replace=false){
    for(const key of albumFields){
      const field=input('album-'+key);
      if(!editedAlbum.has(key)&&album[key]&&(replace||!field.value.trim()))field.value=album[key];
    }
  }
  function render(){
    const split=input('mode').value==='cue';
    $('[data-cue-field]').hidden=!split;
    filesBox.innerHTML=files.map((file,index)=>'<li><span>'+escape(file.name)+'</span>'+
      (split?'':'<div class="actions">'+(index>0?button('上移','up-'+index,'quiet'):'')+
        (index<files.length-1?button('下移','down-'+index,'quiet'):'')+'</div>')+'</li>').join('');
    tracksBox.innerHTML=rows.length?'<table><thead><tr><th>轨号</th><th>音频来源</th><th>起止时间</th><th>曲名</th><th>演出者</th><th>作曲家</th><th>输出文件名</th></tr></thead><tbody>'+
      rows.map((row,i)=>'<tr data-row="'+i+'"><td>'+two(row.number)+'</td><td>'+escape(row.source)+'</td><td>'+escape(split?row.start+' – '+(row.end||'文件结束'):'完整文件')+'</td>'+
        [['title','曲名'],['artist','演出者'],['composer','作曲家'],['filename','文件名（不含扩展名）']].map(([key,label])=>
          '<td><input data-key="'+key+'" maxlength="280" aria-label="第 '+two(row.number)+' 轨'+label+'" value="'+escape(row[key]??(key==='filename'?cdFilename(row,i):''))+'"></td>').join('')+'</tr>').join('')+
      '</tbody></table>':'<p class="hint">'+(split?'选择音频和 CUE 后显示分轨时间。':'选择独立音轨后可调整顺序。')+'</p>';
  }
  function rebuild(preserve=false){
    const previous=rows;
    rows=[];
    cueNotice='';
    try{
      if(input('mode').value==='cue'){
        if(files.length&&cueText){
          const cue=parseCue(cueText,files);
          rows=cue.tracks;
          fillAlbum(cue.album);
        }
      }else{
        rows=files.map((file,index)=>{
          const old=preserve?previous[index]:null;
          return {title:old?.title||'',artist:old?.artist||'',composer:old?.composer||'',filename:old?.filename,number:index+1,source:file.name,start:'00:00',end:''};
        });
      }
    }catch(error){cueNotice=error.message;}
    if(modelTracks)fillUntitledTracks(modelTracks);
    render();
    say(cueNotice||(rows.length?'已列出 '+rows.length+' 轨。请核对文件对应关系和曲目。':'请选择抓轨音频及所需的 CUE。'));
  }
  function fillUntitledTracks(parsed){
    if(parsed.issues.length||!parsed.numbered.size&&parsed.ordered.length!==rows.length)return 0;
    let matched=0;
    for(const [index,row] of rows.entries()){
      if(row.title?.trim())continue;
      const candidate=parsed.numbered.size?parsed.numbered.get(row.number):parsed.ordered[index];
      if(!candidate?.title)continue;
      row.title=candidate.title;
      if(!row.composer?.trim()&&candidate.composer)row.composer=candidate.composer;
      matched++;
    }
    return matched;
  }
  function applyModelResult(text){
    if(!dialog.open)return;
    input('text').value=text;
    modelTracks=readCdText(text);
    fillAlbum(modelTracks.album,true);
    const matched=fillUntitledTracks(modelTracks);
    render();
    const hasAlbum=Object.values(modelTracks.album).some(Boolean);
    say((hasAlbum?'已填入模型识别的专辑标签。':'模型未返回明确的专辑标签。')+
      (matched?' 已匹配 '+matched+' 条空白曲目。':'')+'核对后可直接点「导入分轨页」。');
  }
  function matchText(){
    if(!input('text').value.trim())throw Error('请先输入文字或识别封面。');
    const parsed=readCdText(input('text').value);
    fillAlbum(parsed.album,true);
    if(!rows.length){say(cueNotice||'已读取专辑信息。选择抓轨文件后可匹配曲名。');return;}
    if(!parsed.numbered.size&&parsed.ordered.length!==rows.length){
      say('文字识别出 '+parsed.ordered.length+' 条曲目，音频有 '+rows.length+' 轨。请为文字加上实际轨号，再匹配。');
      return;
    }
    let matched=0;
    rows.forEach((row,i)=>{
      const name=parsed.numbered.get(row.number)||parsed.ordered[i];
      if(!name?.title)return;
      matched++;
      row.title=name.title;
      if(name.composer&&!row.composer)row.composer=name.composer;
    });
    render();
    say('已匹配 '+matched+' / '+rows.length+' 轨。请逐轨核对。 '+parsed.issues.join(' '));
  }
  async function artworkFile(field){
    const file=input(field).files[0];
    if(!file)return null;
    if(artwork.has(file))return artwork.get(file);
    const converted=await prepareArtwork(file,convertArtwork);
    artwork.set(file,converted);
    return converted;
  }
  async function scan(){
    const selected=[];
    for(const key of ['front','back']){
      const file=await artworkFile(key);
      if(file)selected.push({file,text:'',error:''});
    }
    if(!selected.length)throw Error('请选择封面或封底图片。');
    controller=new AbortController();
    await recognizePages(selected,(page,progress)=>say(page?'正在识别第 '+(selected.indexOf(page)+1)+' 张图片'+(progress==null?'…':' '+progress+'%'):'正在加载图片识别…'),controller.signal);
    if(!dialog.open||controller.signal.aborted)return;
    const recognized=selected.map(page=>page.text).filter(Boolean).join('\n\n');
    if(!recognized)throw Error('图片中未识别到可用文字。');
    const text=input('text');
    text.value=[text.value.trim(),recognized].filter(Boolean).join('\n\n').slice(0,50000);
    fillAlbum(readCdText(text.value).album);
    say('已提取专辑标签及识别文字。请核对，再点击「匹配文字到音轨」。');
  }
  function importRows(){
    if(cueNotice)throw Error(cueNotice);
    if(!rows.length)throw Error('请先准备抓轨文件。');
    if(existingTracks&&!input('replace').checked)throw Error('请确认是否替换分轨页现有曲目。');
    const album=Object.fromEntries(albumFields.map(key=>[key,input('album-'+key).value.trim()]));
    const seen=new Set();
    const tracks=rows.map((row,index)=>{
      const filename=row.filename?.trim()||cdFilename(row,index);
      if(!row.title?.trim())throw Error('请填写第 '+two(row.number)+' 轨曲名。');
      if(!outputFilenameValid(filename+'.flac')||seen.has(filename.toLowerCase()))throw Error('第 '+two(row.number)+' 轨的输出文件名无效或重复。');
      seen.add(filename.toLowerCase());
      return {...row,filename,tags:{Title:row.title.trim(),Track:two(row.number),Artist:row.artist||album.artist,
        Album:album.title,Composer:row.composer||album.composer,Date:album.date||row.date}};
    });
    return {tracks,album};
  }
  tracksBox.addEventListener('input',event=>{
    const {target}=event;
    const key=target.dataset.key;
    if(!key)return;
    const tr=target.closest('[data-row]');
    const index=Number(tr.dataset.row),row=rows[index];
    row[key]=target.value;
    if(key==='title'&&!row.filename)tr.querySelector('[data-key="filename"]').value=cdFilename(row,index);
  });
  input('mode').addEventListener('change',()=>rebuild());
  input('cue').addEventListener('change',async event=>{
    const file=event.target.files[0];
    cueText='';
    if(!file){rebuild();return;}
    if(file.size>2000000){rebuild();say('CUE 文件过大。');return;}
    setBusy(true);
    try{cueText=decodeCue(await file.arrayBuffer());rebuild();}
    catch(error){say(error.message);}
    finally{setBusy(false);}
  });
  dialog.addEventListener('click',async event=>{
    const action=event.target.closest('[data-cd]')?.dataset.cd;
    if(action==='close'){dialog.close();return;}
    if(!action||busy)return;
    setBusy(true);
    try{
      if(action==='audio'){
        const picked=await chooseFiles();
        if(picked.length&&dialog.open){files=sortCdFiles(picked);rebuild();}
      }
      if(action==='ocr')await scan();
      if(action==='llm')await openLlmFiles({
        task:'cd',text:input('text').value,files:['front','back','cue'].map(key=>input(key).files[0]).filter(Boolean),
        apply:applyModelResult
      });
      if(action==='match')matchText();
      if(action.startsWith('up-')||action.startsWith('down-')){
        const [direction,value]=action.split('-');
        const i=Number(value),next=i+(direction==='up'?-1:1);
        [files[i],files[next]]=[files[next],files[i]];
        rebuild(true);
      }
      if(action==='apply'){
        const snapshot=importRows();
        const cover=await artworkFile('front');
        const backCover=await artworkFile('back');
        if(dialog.open){apply({...snapshot,files,cover,backCover});dialog.close();}
      }
    }catch(error){if(dialog.open)say(error.message);}
    finally{if(dialog.open)setBusy(false);}
  });
  dialog.addEventListener('close',()=>{controller?.abort();dialog.remove();},{once:true});
  document.body.append(dialog);
  render();
  dialog.showModal();
}
