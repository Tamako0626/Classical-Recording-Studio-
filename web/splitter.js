import {artworkTypes,prepareArtwork} from './artwork.js';
import {effectsForm,limiterForm,readEffects} from './audio-effects.js';
import {attachWorkModel} from './llm-settings.js';
import {llmConfig,llmRecognize} from './llm-settings.js';
import {openLlmFiles} from './llm-files.js';
import {expandMovementTrack,findMovementList,modelMovementList,movementSearch,movementTitles,searchResultMovements} from './work-movements.js';
import {modelSessionReview,normalizeSession,sessionFolderName} from './session-english.js';
import {batchTrackFilenames,englishProgramTrack} from './program-english.js';
import {openProgramImport} from './program-import.js';
import {blankProject,blankTrack,defaults,tags} from './seed.js';
import {batchSessionTags,makePlan,outputFilenameValid,outputValid,sourceValid,timeSeconds,filenameFor} from './script.js';
import {openCdImport} from './cd-import.js';
import {openCoverStudio} from './cover-studio.js';

import {parseWorkQuery,matchWorks,mergeWorks} from './work-catalog.js';

const desktop=window.studioNative;
const coverFormats='PNG / JPG'+(desktop?.convertCdArtwork?' / HEIC / HEIF':'');
const audioTypes={FLAC:'audio/flac',WAV:'audio/wav',MP3:'audio/mpeg',MKV:'audio/x-matroska'};
const sessions={name:'场次 / 专辑名',date:'演出日期',performer:'演奏家',conductor:'指挥家',composer:'作曲家',orchestra:'乐团',city:'城市',venue:'音乐厅',origin:'录音来源'};
const field=(name,label,value='',type='text')=>'<label class="field"><span>'+label+'</span><input name="'+name+'" type="'+type+'" value="'+escape(value)+'" maxlength="280"></label>';
const escape=value=>String(value??'').replace(/[&<>"']/g,s=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[s]));
const btn=(label,key,cls='',id='')=>'<button type="button" class="button '+cls+'" data-split="'+key+'" data-key="'+escape(id)+'">'+label+'</button>';
const select=(name,label,choices,value)=>'<label class="field"><span>'+label+'</span><select name="'+name+'">'+choices.map(([id,text])=>'<option value="'+id+'"'+(id===String(value)?' selected':'')+'>'+text+'</option>').join('')+'</select></label>';
const query=(el,selector)=>el.querySelector(selector);
const stamp=seconds=>{
  const millis=Math.round(Math.max(0,seconds)*1000);
  const h=Math.floor(millis/3600000),m=Math.floor(millis/60000)%60,s=Math.floor(millis/1000)%60,ms=millis%1000;
  return [h,m,s].map(x=>String(x).padStart(2,'0')).join(':')+(ms?'.'+String(ms).padStart(3,'0'):'');
};
function firstTrack(start='00:00'){
  const track=blankTrack();
  return {...track,start};
}
function assignEnds(tracks,join){
  const nextStarts=new Map();
  return tracks.slice().reverse().map(track=>{
    const end=track.end||(join&&track.source?nextStarts.get(track.source)||'':'');
    nextStarts.set(track.source,track.start);
    return {...track,end};
  }).reverse();
}
class Engine {
  constructor(progress) {
    this.worker=new Worker('/ffmpeg-worker.js');
    this.pending=new Map();
    this.next=0;
    this.worker.onmessage=({data})=>{
      if(data.kind){progress(data.seconds,data.kind);return;}
      const request=this.pending.get(data.id);
      if(!request)return;
      this.pending.delete(data.id);
      if(data.error)request.reject(Error(data.error));
      if(!data.error)request.resolve(data.value);
    };
    this.worker.onerror=()=>this.close('FFmpeg 工作线程意外退出。请重试，或改用项目中的 PowerShell 脚本。');
  }
  send(action,payload){
    if(!this.worker)return Promise.reject(Error('FFmpeg 工作线程已关闭。'));
    return new Promise((resolve,reject)=>{
      const id=++this.next;
      this.pending.set(id,{resolve,reject});
      this.worker.postMessage({id,action,payload});
    });
  }
  close(reason='已停止处理。'){
    this.worker?.terminate();
    this.worker=null;
    for(const {reject} of this.pending.values())reject(Error(reason));
    this.pending.clear();
  }
}
export function createSplitter({getRule,getProjects,saveProject,saveResult,finishProject,openProject,openLocalScript,newSession,existing}){
  const root=document.createElement('section');
  const state={project:existing?{...existing}:blankProject(),tracks:existing?.tracks.length?structuredClone(existing.tracks):[firstTrack()],file:null,cover:null,backCover:null,backCoverFile:existing?.backCoverFile||'',coverUrl:'',audioUrl:'',offset:0,native:true,results:[],busy:false,stop:false,engine:null,folder:null,cache:null,cacheName:'',savedId:existing?.id||'',trackDuration:0};
  let inputs=[],activeInputs=[],mergedFile=null,mediaSave,preparingArtwork,restoring=false,disposed=false;
  const refs=existing?.localMedia;
  let missingAudio=[],missingMerged=refs?.merged||null;
  const imageIds={cover:refs?.cover||'',backCover:refs?.backCover||''};
  let mergedParts=[];
  let progressFrame=0,idleWaiter,savedDraft;
  root.innerHTML='<div class="page-head"><div><h1>整场录音分轨</h1><p>试听原始录音，标记起止点，使用 FFmpeg 分轨与处理。</p></div>'+'<div class="actions">'+(desktop?btn('CD 抓轨整理','cd-import','primary'):'')+btn('新建分轨场次','new-session')+'</div></div>'+
    '<section class="card pad"><div class="card-head"><h2>1. 导入录音</h2><span class="badge gray">本机处理</span></div><div class="form-grid">'+
    (desktop?'<div class="span2">'+btn('导入录音文件','select-audio','primary')+btn('识别录音标签','llm-audio','quiet')+'<p class="hint">支持 WAV、FLAC、APE、MP3、MKV 多选。</p></div>':'<label class="field span2"><span>录音文件 · WAV / FLAC / MP3 / MKV（可多选）</span><input type="file" data-picker="audio" accept=".wav,.flac,.mp3,.mkv" multiple></label>')+'</div>'+
    (desktop?.restoreProjectMedia?'<p data-media-status class="hint" role="status"></p><ul class="recording-inputs" data-missing-media hidden></ul>':'')+
    '<div data-merge-panel hidden><ol data-inputs class="recording-inputs"></ol><div class="actions">'+btn('按此顺序拼接','merge','primary')+btn('下载拼接文件','download-merged')+'</div><p class="hint" data-merge-status role="status"></p></div>'+
    '<label class="field section-gap"><span>当前试听文件</span><select name="preview-source"></select></label>'+
    '<div class="split-media section-gap"><div><div class="hint" data-info>请选择原始音频文件。</div><audio controls preload="metadata" aria-label="试听整场录音"></audio>'+
    '<div class="actions section-gap"><span class="mono" data-clock>00:00:00</span>'+btn('↶ 后退 5 秒','back-five')+btn('↷ 前进 5 秒','forward-five')+btn('从指定时间试听 20 秒','preview','quiet')+btn('返回整场试听','native','quiet')+'</div><div class="split-preview" hidden>'+field('preview-time','试听起点','00:00')+btn('播放片段','listen')+'</div></div></div>'+
    '<p class="hint">'+(desktop?'使用本机 FFmpeg 和 FFprobe，将分轨文件写入专辑子文件夹。':'音频与封面留在本机。首次运行会下载约 32 MB 的 FFmpeg 引擎。浏览器无法播放整场文件时，可用指定时间试听。')+'</p></section>'+
    '<section class="card pad"><div class="card-head"><h2>2. 场次信息</h2><div class="actions">'+btn('一键转英文并核对','session-english')+btn('识别场次信息','recognize-session','primary')+btn('导入节目单','program')+'</div></div><div class="form-grid">'+Object.entries(sessions).map(([key,label])=>field(key,label,existing?.[key]??(key==='origin'?'现场自录':''),key==='date'?'date':'')).join('')+field('album-date','专辑标签日期 / 年份（CD）',existing?.albumDate||'')+'</div><div data-session-result class="session-result hint" role="status"></div></section>'+
    '<section class="card pad"><div class="card-head"><h2>专辑封面</h2></div><div class="split-media"><div class="actions">'+btn('上传场照制作封面','make-cover','primary')+'<label class="field"><span>或上传现成封面 · '+coverFormats+'</span><input type="file" data-picker="cover" accept="'+artworkTypes(desktop?.convertCdArtwork)+'"></label>'+btn('下载当前封面','local-cover','quiet')+btn('移除封面','remove-cover','quiet')+(desktop?btn('识别封面文字','llm-cover','quiet'):'')+'</div><img hidden alt="专辑封面预览"></div><p data-cover-status class="hint"></p>'+
    (desktop?'<div class="actions section-gap"><label class="field"><span>封底（可选） · '+coverFormats+'</span><input type="file" data-picker="back-cover" accept="'+artworkTypes(desktop?.convertCdArtwork)+'"></label>'+btn('移除封底','remove-back-cover','quiet')+'</div><p class="hint" data-back-cover-status></p>':'')+'</section>'+
    '<section class="card"><div class="card-head pad" style="margin:0"><h2>3. 曲目与时间点</h2><div class="actions">'+btn('按场次更新全部标签','batch-tags')+btn('一键匹配全部文件名','match-all')+btn('＋ 添加曲目','add','primary')+btn('＋ 从播放位置添加','mark-add')+'</div></div>'+
    '<div class="split-options pad"><label><input type="checkbox" name="join" checked> 终点留空时接到同一文件的下一轨起点，末轨到该文件结束</label><p class="hint">每轨可选择不同的音频文件，起止点以所选文件为准。可输入 18:22 或 01:18:22.500。点击「取播放点」填写当前时间，点击「试听」从本轨起点播放。曲目名可先填简称，再点「匹配文件名」。匹配成功会替换当前文件名，结果可在曲目下方直接编辑。</p><p data-batch-status class="hint" role="status"></p><div data-tag-sync class="actions"></div></div>'+
    '<div class="table-wrap"><table class="split-tracks"><thead><tr><th>顺序</th><th>曲目名称</th><th>音频来源</th><th>起点</th><th>终点</th><th>试听与标签</th></tr></thead><tbody></tbody></table></div></section>'+
    '<section class="card pad"><div class="card-head"><h2>4. 输出设置</h2></div><div class="form-grid">'+
    select('format','输出格式',Object.keys(audioTypes).map(k=>[k,k]),'FLAC')+select('rate','采样率',[['44100','44.1 kHz'],['48000','48 kHz'],['96000','96 kHz']],'48000')+
    '<div class="gain-controls"><label class="field"><span>增益（dB）</span><input type="number" name="gain" value="0" min="-60" max="30" step="0.1"></label>'+limiterForm(existing?.settings?.effects)+'</div>'+
    [['width','声场宽度（%）',100,0,250,1],['reverb','混响（%）',0,0,100,1]].map(([name,label,value,min,max,step])=>'<label class="field"><span>'+label+'</span><input type="number" name="'+name+'" value="'+value+'" min="'+min+'" max="'+max+'" step="'+step+'"></label>').join('')+
    field('rule','英文命名规则',getRule())+'</div>'+effectsForm(existing?.settings?.effects)+(desktop?'<div class="actions section-gap">'+btn('试听处理效果','preview-effects')+'</div>':'')+'<p class="hint">默认 48 kHz。WAV 选了封面时转为 FLAC。WAV 只支持部分标签。宽度调整适用于双声道。大幅提高增益时可勾选「启用防爆音限幅」，默认峰值上限为 −1 dBFS。</p>'+
    '<div class="actions section-gap">'+btn('选择保存位置','folder')+'<span class="hint" data-folder>运行时可选择保存位置</span></div>'+
    '<div class="section-gap">'+field('album-folder','专辑文件夹名称')+'<p class="hint" data-album-path aria-live="polite"></p><p class="hint">留空时按照场次信息自动命名。输入自定义名称可覆盖。</p></div>'+
    '<div class="actions section-gap">'+btn('运行 FFmpeg','run','primary')+btn('生成本机 PowerShell 脚本','local-script')+btn('停止处理','stop')+btn('保存到录音项目','save')+btn('打开已保存项目 ↗','open','quiet')+'</div>'+
    '<div class="alert" data-browser-limit hidden><p data-limit-note role="status"></p><div class="actions">'+btn('改用本机 PowerShell','local-script','primary')+btn('下载脚本所用封面','local-cover')+btn('下载脚本所用拼接录音','local-merged')+'</div></div>'+
    '<div class="alert" role="status" data-message></div><div data-progress hidden><progress max="100" value="0" aria-label="分轨进度"></progress> <span data-progress-text></span></div></section>'+
    '<section class="card pad"><div class="card-head"><h2>分轨结果</h2>'+(desktop?.selectVideoSources?btn('制作封面视频','cover-videos','primary'):'')+'</div><div data-results class="hint">生成的曲目将在这里显示。</div></section>';
  const audio=query(root,'audio');
  const panel=query(root,'[data-merge-panel]');
  const originalCover=existing?.coverFile||'';
  let coverRequired=!!originalCover&&existing.settings?.cover;
  if(existing){
    query(root,'[data-info]').textContent=(existing.sources.length?'请重新导入 '+existing.sources.join('、')+'。':'请选择此场次的原始音频文件。')+
      (existing.recordingParts?.length?'也可重新导入原始片段并按原顺序拼接。':'');
    for(const key of ['format','rate','gain','width','reverb'])query(root,'[name="'+key+'"]').value=existing.settings[key];
    query(root,'[name="rule"]').value=existing.namingRule||getRule();
  }
  const tbody=query(root,'tbody');
  const message=text=>{query(root,'[data-message]').textContent=text;};
  const ctrl=name=>query(root,'[name="'+name+'"]');
  const sessionValues=()=>Object.fromEntries(Object.keys(sessions).map(key=>[key,ctrl(key).value.trim()]));
  const albumField=ctrl('album-folder');
  ctrl('join').checked=existing?.autoJoin!==false;
  if(existing?.output&&(existing.autoOutput===false||
    existing.autoOutput===undefined&&existing.output!=='Processed'&&existing.output!==sessionFolderName(existing)))albumField.value=existing.output;
  function updateAlbumPath(){
    const automatic=sessionFolderName(sessionValues());
    albumField.placeholder=automatic;
    query(root,'[data-album-path]').textContent=(state.folder?state.folder.name.replace(/[\\/]+$/u,'')+'\\':'')+(albumField.value.trim()||automatic);
  }
  let recordingDuration=null;
  const position=()=>state.offset+(audio.readyState?audio.currentTime:0);
  function updateClock(){
    const ready=audio.readyState>0&&Number.isFinite(audio.duration);
    for (const key of ['back-five','forward-five']) query(root,'[data-split="'+key+'"]').disabled=state.busy||!ready;
    query(root,'[data-clock]').textContent=stamp(position())+(state.native&&Number.isFinite(audio.duration)?' / '+stamp(audio.duration):'');
  }
  for(const event of ['timeupdate','loadedmetadata','seeked','emptied','durationchange'])audio.addEventListener(event,updateClock);
  audio.addEventListener('error',()=>{
    updateClock();
    if(state.file&&state.native)query(root,'[data-info]').textContent=state.file.name+' · 浏览器无法直接试听，请使用指定时间试听。';
  });
  function useAudio(blob,offset=0,native=true,play=false){
    if(state.audioUrl)URL.revokeObjectURL(state.audioUrl);
    state.audioUrl=blob.url?'':URL.createObjectURL(blob);
    state.offset=offset;state.native=native;
    audio.src=blob.url||state.audioUrl;updateClock();
    if(play)audio.play().catch(()=>{});
  }
  const savedParts=Array.isArray(existing?.recordingParts)?existing.recordingParts:[];
  const canMerge=()=>inputs.length>1&&
    (inputs.length!==activeInputs.length||inputs.some((file,index)=>file!==activeInputs[index]));
  const loadedFiles=()=>[...inputs,...(mergedFile?[mergedFile]:[])];
  const findFile=name=>loadedFiles().find(file=>file.name.toLowerCase()===name?.toLowerCase());
  const trackFile=track=>findFile(track.source);
  function sourceOptions(value,files){
    return '<option value="">请选择音频文件</option>'+files.map(file=>
      '<option value="'+escape(file.name)+'"'+(file.name===value?' selected':'')+'>'+escape(file.name)+'</option>').join('')+
      (value&&!files.some(file=>file.name===value)?'<option value="'+escape(value)+'" selected>'+escape(value)+'（待重新导入）</option>':'');
  }
  function activateRecording(file,duration=null){
    state.file=file;recordingDuration=duration;useAudio(file);
    ctrl('preview-source').value=file.name;
    query(root,'[data-info]').textContent=file.name+' · '+(file.size/1048576).toFixed(1)+' MB'+(duration?' · '+stamp(duration):'');
    if(!ctrl('name').value)ctrl('name').value=(inputs[0]?.name||file.name).replace(/\.[^.]+$/,'');
    updateAlbumPath();
  }
  function renderInputs(){
    panel.hidden=!inputs.length;
    query(panel,'[data-inputs]').innerHTML=inputs.map((file,i)=>'<li><span>'+escape(file.name)+'</span><div class="actions">'+
      (i>0?btn('上移','input-up','quiet',String(i)):'')+(i<inputs.length-1?btn('下移','input-down','quiet',String(i)):'')+
      btn('移除','input-remove','quiet',String(i))+'</div></li>').join('');
    query(panel,'[data-merge-status]').textContent=mergedFile
      ? '已拼接为 '+mergedFile.name+'。可在分轨表中选用拼接文件或原始片段。'
      : '可直接选用各段录音，也可按列表顺序拼接。';
    ctrl('preview-source').innerHTML=sourceOptions(state.file?.name,loadedFiles());
    rows();
    controls();
  }
  function updateInputs(files){
    inputs=files;
    for(const track of state.tracks){
      const match=findFile(track.source);
      if(match)track.source=match.name;
      if(!track.source&&loadedFiles().length===1&&(!existing?.sources.length||existing.sources.length===1&&inputs[0]?.name.toLowerCase()===existing.sources[0].toLowerCase())){
        track.source=loadedFiles()[0].name;
      }
    }
    const current=findFile(state.file?.name)||loadedFiles()[0];
    if(current&&current!==state.file)activateRecording(current);
    if(!current){
      if(state.audioUrl)URL.revokeObjectURL(state.audioUrl);
      state.audioUrl='';state.file=null;recordingDuration=null;
      audio.removeAttribute('src');audio.load();
      query(root,'[data-info]').textContent='请选择原始音频文件。';
    }
    renderInputs();
  }
  function addInputs(selected){
    if(!selected.length)return;
    if(mergedFile&&selected.some(item=>item.name.toLowerCase()===mergedFile.name.toLowerCase())){
      message('导入文件与当前拼接文件同名，请先修改导入文件名。');return;
    }
    const matched=new Set(selected.map(file=>file.name.toLowerCase()));
    missingAudio=missingAudio.filter(ref=>!matched.has(ref.name.toLowerCase()));
    renderMissingMedia();
    const unique=new Map([...inputs,...selected.sort((a,b)=>a.name.localeCompare(b.name,undefined,{numeric:true}))].map(item=>[item.name.toLowerCase(),item]));
    const next=[...unique.values()];
    if(savedParts.length){
      const order=new Map(savedParts.map((name,index)=>[name.toLowerCase(),index]));
      next.sort((a,b)=>(order.get(a.name.toLowerCase())??Infinity)-(order.get(b.name.toLowerCase())??Infinity));
    }
    updateInputs(next);
  }
  async function mergeInputs(){
    if(mergedFile&&state.tracks.some(track=>track.source===mergedFile.name))throw Error('请先把引用旧拼接文件的曲目切换到其他来源，再重新拼接。');
    state.busy=true;state.stop=false;controls();audio.pause();message('');
    try{
      updateProgress('正在检查音频并按列表顺序拼接…',0);
      const rebuild=savedParts.length===inputs.length&&inputs.every((file,index)=>file.name.toLowerCase()===savedParts[index].toLowerCase());
      const originalName=existing?.sources[0];
      const occupied=new Set([...inputs.map(file=>file.name),...state.project.sources.filter(name=>!rebuild||name!==originalName)].map(name=>name.toLowerCase()));
      let name=rebuild&&/\.flac$/iu.test(originalName)?originalName:'joined-recording.flac';
      for(let suffix=2;occupied.has(name.toLowerCase());suffix++)name='joined-recording-'+suffix+'.flac';
      const payload=desktop?{ids:inputs.map(file=>file.id),name}:inputs;
      const {bytes,duration,file}=await engine().send('concatenate',payload);
      mergedFile=file||new File([bytes],name,{type:'audio/flac'});
      missingMerged=null;
      activeInputs=inputs.slice();
      mergedParts=inputs.map(file=>file.name);
      renderMissingMedia();
      for(const track of state.tracks){
        if(!track.source)track.source=mergedFile.name;
      }
      activateRecording(mergedFile,duration);
      renderInputs();
    }finally{state.busy=false;state.trackDuration=0;controls();}
  }
  function showCover(){
    query(root,'[data-cover-status]').textContent=state.cover?'已载入 '+state.cover.name+'。':coverRequired?'项目记录了 '+originalCover+'。':'';
  }
  function applyCover(file,id=''){
    imageIds.cover=id;
    state.cover=file;
    if(state.coverUrl)URL.revokeObjectURL(state.coverUrl);
    state.coverUrl=file?URL.createObjectURL(file):'';
    const img=query(root,'img');
    img.hidden=!file;
    if(file){img.src=state.coverUrl;img.title=file.name;}
    if(!file)img.removeAttribute('src');
    showCover();
    controls();
  }
  function showBackCover(){
    const status=query(root,'[data-back-cover-status]');
    if(!status)return;
    status.textContent=state.backCover?'已选择 '+state.backCover.name+'，导出时保存到专辑文件夹。':
      state.backCoverFile?'项目记录了 '+state.backCoverFile+'。需要一并导出时可重新选择。':'可留空。';
    query(root,'[data-split="remove-back-cover"]').hidden=!state.backCoverFile;
  }
  function applyBackCover(file,id=''){
    imageIds.backCover=id;
    state.backCover=file;
    state.backCoverFile=file?.name||'';
    showBackCover();
  }
  async function nativeCall(promise){
    const response=await promise;
    if(response.error)throw Error(response.error);
    return response.value;
  }
  const convertArtwork=desktop?.convertCdArtwork?bytes=>nativeCall(desktop.convertCdArtwork(bytes)):undefined;
  async function chooseFolder(){
    if(desktop){
      const folder=await nativeCall(desktop.selectFolder());
      if(!folder)return;
      state.folder=folder;
    }else{
      if(!window.showDirectoryPicker)throw Error('当前浏览器视图无法选择输出目录。处理后可逐轨下载。');
      state.folder=await window.showDirectoryPicker({mode:'readwrite'});
    }
    query(root,'[data-folder]').textContent=state.folder.name;
    updateAlbumPath();
  }
  function downloadFile(file){
    const link=document.createElement('a');
    link.href=file.url||URL.createObjectURL(file);link.download=file.name;link.click();
    if(!file.url)setTimeout(()=>URL.revokeObjectURL(link.href),30000);
  }
  const outputExtension=()=>(ctrl('format').value==='WAV'&&(state.cover||coverRequired)?'FLAC':ctrl('format').value).toLowerCase();
  function rows(){
    tbody.innerHTML=state.tracks.map((t,i)=>'<tr data-row="'+t.id+'"><td>'+String(i+1).padStart(2,'0')+'</td>'+
      '<td><input data-track="Title" aria-label="第 '+(i+1)+' 轨曲目名称" value="'+escape(t.tags.Title)+'" placeholder="例如 Piano Sonata No. 11" maxlength="280">'+'<label class="field split-output-name"><span>输出文件名（不含扩展名）</span><input data-track="filename" aria-label="第 '+(i+1)+' 轨输出文件名" value="'+escape(t.filename)+'" placeholder="留空时按命名规则生成" maxlength="280"></label>'+'</td>'+
      '<td><select class="track-source" data-track="source" aria-label="第 '+(i+1)+' 轨音频来源">'+sourceOptions(t.source,loadedFiles())+'</select></td>'+
      ['start','end'].map(key=>'<td><div class="split-time"><input data-track="'+key+'" aria-label="第 '+(i+1)+' 轨'+(key==='start'?'起点':'终点')+'" value="'+escape(t[key])+'" placeholder="'+(key==='start'?(t.awaitingCue?'待标记':'00:00'):'自动')+'" maxlength="24">'+btn('取播放点','mark-'+key,'quiet',t.id)+'</div></td>').join('')+
      '<td><div class="actions">'+btn('试听','seek','quiet',t.id)+btn('匹配文件名','match','',t.id)+btn('展开乐章／分曲','movements','quiet',t.id)+btn('编辑标签','tags','quiet',t.id)+(state.tracks.length>1?btn('删除','remove','quiet',t.id):'')+'</div></td></tr>').join('');
  }
  function results(){
    query(root,'[data-results]').innerHTML=state.results.length?'<ul class="split-results">'+state.results.map((r,i)=>'<li><div><strong>'+escape(r.job.target)+'</strong><small>'+escape(r.error||r.note||stamp(r.duration))+'</small></div>'+(r.nativeId?btn('在文件夹中显示','reveal','',String(i)):r.blob||r.handle?btn('下载','download','',String(i)):'')+'</li>').join('')+'</ul>':'生成的曲目将在这里显示。';
  }
  function controls(){
    if(!state.busy&&idleWaiter){idleWaiter();idleWaiter=null;}
    root.querySelectorAll('button,input,select').forEach(el=>{
      const action=el.dataset.split;
      el.disabled=action==='stop'?!state.busy:action==='open'?!state.savedId:action==='download'?false:state.busy;
      if(action==='merge')el.disabled=state.busy||!canMerge();
      if(restoring||mediaSave||preparingArtwork)el.disabled=true;
      if(action==='download-merged')el.hidden=!mergedFile;
      if(action==='local-cover')el.hidden=!state.cover;
      if(action==='local-script'&&!el.closest('[data-browser-limit]'))el.hidden=!!desktop;
      if(action==='local-merged')el.hidden=!mergedFile||!state.tracks.some(track=>track.source===mergedFile.name);
    });
    query(root,'[data-progress]').hidden=!state.busy;
    updateClock();
  }
  function updateProgress(text,value){
    query(root,'[data-progress-text]').textContent=text;
    query(root,'progress').value=value;
  }
  function progress(seconds,kind){
    if(kind==='merge-duration'){state.trackDuration=seconds||0;return;}
    if(progressFrame)return;
    progressFrame=requestAnimationFrame(()=>{
      progressFrame=0;
      if(state.busy&&state.trackDuration>0)query(root,'progress').value=Math.min(99,100*seconds/state.trackDuration);
    });
  }
  function engine(){
    if(nativeEngine)return nativeEngine;
    if(!state.engine?.worker)state.engine=new Engine(progress);
    return state.engine;
  }
  const nativeEngine=desktop&&{
    send:(action,payload)=>nativeCall(desktop.process(action,payload)),
    close:()=>desktop.stop()
  };
  const clearProgress=desktop?.onProgress((kind,seconds)=>{if(root.isConnected)progress(seconds,kind);});
  function coverVideoDialog(){
    const dialog=document.createElement('dialog');
    dialog.className='program-dialog';
    dialog.setAttribute('aria-label','制作封面视频');
    dialog.innerHTML='<div class="dialog-head"><h2>制作封面视频</h2>'+btn('关闭','dismiss','quiet')+'</div>'+
      '<div class="dialog-body"><div class="form-grid">'+
      '<label class="field"><span>封面 · '+coverFormats+'</span><input type="file" accept="'+artworkTypes(convertArtwork)+'" data-video-cover><small data-video-cover-name></small></label>'+
      select('video-mode','封装格式',[['auto','自动：高采样率无损音频用 MKV，其余用 MP4'],['mp4','全部生成 MP4（AAC）'],['mkv48','全部生成 MKV（不足 48 kHz 时升频）']],'auto')+'</div>'+
      '<p class="hint">每条选中的音轨生成一个 1080p 静态封面视频。自动模式将 48 kHz 及以上的无损音频封装为 MKV，保留原采样率；升频只改变输出规格。</p>'+
      '<div class="actions">'+btn('添加已分轨音频','add-video-sources')+btn('全选','pick-video-sources','quiet')+btn('清空勾选','clear-video-sources','quiet')+'</div>'+
      '<ul class="split-results video-source-list" data-video-sources></ul>'+
      '<p class="hint" role="status" data-video-status></p><div data-video-results></div></div>'+
      '<div class="dialog-foot">'+btn('停止封装','stop-videos')+btn('选择目录并生成','render-videos','primary')+'</div>';
    const $=part=>query(dialog,part);
    const available=state.results.filter(result=>result.nativeId&&!result.error).map(result=>({id:result.nativeId,name:result.job.target}));
    const chosen=new Set(available.map(file=>file.id));
    const list=$('[data-video-sources]');
    const status=$('[data-video-status]');
    const results=$('[data-video-results]');
    const picker=$('[data-video-cover]');
    const mode=$('[name="video-mode"]');
    const stop=$('[data-split="stop-videos"]');
    let busy=false;
    $('[data-video-cover-name]').textContent=state.cover?'默认使用 '+state.cover.name:'请选取封面图片。';
    function drawList(){
      list.innerHTML=available.length?available.map((file,i)=>'<li><label><input type="checkbox" value="'+i+'"'+(chosen.has(file.id)?' checked':'')+'> '+escape(file.name)+'</label></li>').join(''):'<li>还没有可用的分轨结果，可从磁盘添加。</li>';
    }
    function lock(value){
      busy=value;
      for(const control of dialog.querySelectorAll('button,input,select'))control.disabled=value;
      stop.disabled=!value;
    }
    dialog.addEventListener('cancel',event=>{if(busy)event.preventDefault();});
    dialog.addEventListener('close',()=>dialog.remove(),{once:true});
    list.onchange=event=>{
      const id=available[Number(event.target.value)]?.id;
      if(!id)return;
      if(event.target.checked)chosen.add(id);
      else chosen.delete(id);
    };
    $('[data-split="dismiss"]').onclick=()=>dialog.close();
    $('[data-split="add-video-sources"]').onclick=async()=>{
      lock(true);stop.disabled=true;
      try{
        const files=await nativeCall(desktop.selectVideoSources());
        for(const file of files.sort((a,b)=>a.name.localeCompare(b.name,undefined,{numeric:true}))){
          available.push(file);
          chosen.add(file.id);
        }
        drawList();
      }catch(error){status.textContent=error.message;}
      finally{lock(false);}
    };
    $('[data-split="pick-video-sources"]').onclick=()=>{for(const file of available)chosen.add(file.id);drawList();};
    $('[data-split="clear-video-sources"]').onclick=()=>{chosen.clear();drawList();};
    stop.onclick=()=>{state.stop=true;nativeEngine.close();stop.disabled=true;status.textContent='正在停止封装…';};
    $('[data-split="render-videos"]').onclick=async()=>{
      const selectedCover=picker.files[0]||state.cover;
      if(!selectedCover){status.textContent='请先选择封面。';return;}
      const ids=available.filter(file=>chosen.has(file.id)).map(file=>file.id);
      if(!ids.length||ids.length>500){status.textContent='请选择 1 至 500 条音轨。';return;}
      lock(true);stop.disabled=true;
      let processing=false;
      try{
        const cover=await prepareArtwork(selectedCover,convertArtwork,{limit:30*1024*1024});
        const folder=await nativeCall(desktop.selectFolder());
        if(!folder)return;
        const bytes=new Uint8Array(await cover.arrayBuffer());
        state.busy=processing=true;state.stop=false;controls();stop.disabled=false;
        results.replaceChildren();
        status.textContent='正在使用本机 FFmpeg 封装 '+ids.length+' 条音轨…';
        const batch=await nativeEngine.send('cover-videos',{folder:folder.id,ids,cover:{name:cover.name,bytes},mode:mode.value});
        const success=batch.results.filter(item=>item.id).length;
        status.textContent=(batch.stopped?'已停止。':'处理完成。')+' 成功 '+success+'，失败 '+(batch.results.length-success)+'。'+(success?' 保存到 '+batch.folder:'');
        const table=document.createElement('ul');table.className='split-results';
        for(const item of batch.results){
          const row=document.createElement('li'),description=document.createElement('div');
          const title=document.createElement('strong');title.textContent=item.target||item.source;
          const detail=document.createElement('small');
          detail.textContent=item.error||item.codec.toUpperCase()+' · '+item.rate/1000+' kHz'+(item.bitDepth?' · '+item.bitDepth+' bit':'')+
            (item.rate!==item.sourceRate?' · 源采样率 '+item.sourceRate/1000+' kHz':'')+(item.target?.endsWith('.mkv')&&!item.sourceLossless?' · 源音频为有损编码':'');
          description.append(title,detail);row.append(description);
          if(item.id){
            const reveal=document.createElement('button');reveal.type='button';reveal.className='button';reveal.textContent='在文件夹中显示';
            reveal.onclick=()=>void nativeCall(desktop.reveal(item.id)).catch(error=>{status.textContent=error.message;});
            row.append(reveal);
          }
          table.append(row);
        }
        results.append(table);
      }catch(error){status.textContent=state.stop?'已停止封装。':error.message;}
      finally{
        if(processing){state.busy=false;controls();}
        lock(false);
      }
    };
    drawList();lock(false);
    document.body.append(dialog);dialog.showModal();
  }
  function outputSettings(){
    const settings={...defaults,cover:!!state.cover||coverRequired,format:ctrl('format').value,rate:Number(ctrl('rate').value)};
    for(const key of ['gain','width','reverb']){
      if(!ctrl(key).value.trim())throw Error('请填写增益、声场宽度和混响。');
      settings[key]=Number(ctrl(key).value);
    }
    settings.effects=readEffects(root);
    return settings;
  }
  function previewEffectsDialog(){
    const settings=outputSettings();
    const tracks=assignEnds(state.tracks,ctrl('join').checked);
    const options=tracks.flatMap((track,index)=>{
      const file=findFile(track.source);
      if(!file||!track.start.trim())return [];
      return [{file,label:'第 '+(index+1)+' 轨 · '+(track.tags.Title||file.name),
        job:{start:timeSeconds(track.start.trim()),end:timeSeconds(track.end.trim())}}];
    });
    if(state.file)options.push({file:state.file,label:'当前录音 · '+state.file.name,job:{start:0,end:null}});
    if(!options.length)throw Error('请先导入录音。');
    const dialog=document.createElement('dialog');
    dialog.className='program-dialog effects-preview-dialog';
    dialog.setAttribute('aria-label','试听处理效果');
    dialog.innerHTML='<div class="dialog-head"><h2>试听处理效果</h2>'+btn('关闭','close-preview','quiet')+'</div>'+
      '<div class="dialog-body"><div class="form-grid">'+select('effect-preview-track','试听来源',options.map(({label},index)=>[String(index),escape(label)]),'0')+
      select('effect-preview-segment','试听片段',[['beginning','开头 20 秒'],['ending','结尾 20 秒']],'beginning')+'</div>'+
      '<p class="hint">短于 20 秒时试听整段。结尾试听会从本轨起点运行滤镜，长录音可能需要等待。</p><p class="hint" role="status" data-effect-preview-status></p>'+
      '<div class="effects-preview-players" hidden><div><strong>原始录音</strong><audio controls preload="metadata" aria-label="试听原始片段"></audio></div>'+
      '<div><strong>应用当前参数后</strong><audio controls preload="metadata" aria-label="试听处理后的片段"></audio></div></div></div>'+
      '<div class="dialog-foot">'+btn('生成对比试听','render-preview','primary')+'</div>';
    const players=query(dialog,'.effects-preview-players');
    const [original,processed]=players.querySelectorAll('audio');
    const trackPicker=query(dialog,'[name="effect-preview-track"]');
    const segmentPicker=query(dialog,'[name="effect-preview-segment"]');
    const startButton=query(dialog,'[data-split="render-preview"]');
    const status=query(dialog,'[data-effect-preview-status]');
    const urls=[];
    let busy=false;
    function clear(){
      for(const audio of [original,processed]){
        audio.pause();
        audio.removeAttribute('src');
        audio.load();
      }
      for(const url of urls.splice(0))URL.revokeObjectURL(url);
      players.hidden=true;
    }
    function pauseOthers(current,other){
      current.addEventListener('play',()=>other.pause());
    }
    pauseOthers(original,processed);
    pauseOthers(processed,original);
    trackPicker.onchange=segmentPicker.onchange=()=>{clear();status.textContent='';};
    query(dialog,'[data-split="close-preview"]').onclick=()=>dialog.close();
    dialog.addEventListener('close',()=>{if(busy)nativeEngine.close();clear();dialog.remove();},{once:true});
    startButton.onclick=async()=>{
      const {file,job}=options[Number(trackPicker.value)];
      busy=startButton.disabled=trackPicker.disabled=segmentPicker.disabled=true;
      clear();
      status.textContent='正在用本机 FFmpeg 生成原始和处理后的试听片段…';
      try{
        const result=await nativeEngine.send('preview-effects',{id:file.id,job,settings,segment:segmentPicker.value});
        if(!dialog.open)return;
        for(const [audio,data] of [[original,result.original],[processed,result.processed]]){
          const url=URL.createObjectURL(new Blob([data],{type:'audio/wav'}));
          urls.push(url);audio.src=url;
        }
        players.hidden=false;
        status.textContent='两段试听使用相同的时间范围：'+stamp(result.start)+' – '+stamp(result.start+result.duration)+'。';
      }catch(error){if(dialog.open)status.textContent=error.message;}
      finally{busy=startButton.disabled=trackPicker.disabled=segmentPicker.disabled=false;}
    };
    audio.pause();
    document.body.append(dialog);
    dialog.showModal();
  }
  function collect(requireTracks=true,requireFiles=requireTracks){
    const p={...state.project,...sessionValues(),albumDate:ctrl('album-date').value.trim()};
    const custom=albumField.value.trim();
    p.output=custom||sessionFolderName(p);
    p.autoOutput=!custom;
    p.autoJoin=ctrl('join').checked;
    if(!outputValid(p.output))throw Error('请输入有效的 Windows 文件夹名称，最多 100 个字符。');
    if(!p.name)throw Error('请填写场次 / 专辑名。');
    const files=loadedFiles();
    if(!files.length&&requireFiles)throw Error('请先导入录音。');
    const names=new Map([...p.sources,...state.tracks.map(t=>t.source).filter(Boolean),...files.map(file=>file.name)].map(name=>[name.toLowerCase(),name]));
    p.sources=[...names.values()];
    if(mergedFile){
      p.sources=[mergedFile.name,...p.sources.filter(name=>name!==mergedFile.name)];
      p.recordingParts=mergedParts;
    }
    p.backCoverFile=state.backCoverFile;
    p.coverFile=state.cover?.name||(coverRequired?originalCover:'');
    p.coverStatus=state.cover?'已准备':coverRequired?'待核对':'无需封面';
    const settings=outputSettings();
    p.settings=settings;
    p.tracks=assignEnds(state.tracks,ctrl('join').checked).map(t=>({...t,start:t.start.trim(),end:t.end.trim(),source:names.get(t.source?.toLowerCase())||'',tags:Object.fromEntries(Object.entries(t.tags).map(([k,v])=>[k,v.trim()]))}));
    const rule=ctrl('rule').value.trim();
    if(!rule)throw Error('请填写命名规则。');
    const {plan,errors}=makePlan(p,settings,rule,p.output);
    if(requireFiles)for(const job of plan.jobs){
      if(job.source&&!findFile(job.source))errors.push('第 '+job.order+' 轨请重新导入录音 '+job.source+'。');
    }
    if(requireTracks&&errors.length)throw Error(errors.join('\n'));
    if(!requireTracks&&p.tracks.length===1)p.tracks=p.tracks.filter(t=>Object.values(t.tags).some(Boolean)||t.filename||t.end||t.start!=='00:00');
    return {project:p,plan,rule};
  }
  let previousSession=null;
  const sessionResult=()=>query(root,'[data-session-result]');
  function normalizeSessionFields(){
    const values=sessionValues();
    const {changes,review,verified}=normalizeSession(values);
    const changed=Object.keys(changes);
    if(changed.length){
      previousSession=Object.fromEntries(changed.map(key=>[key,{before:values[key],after:changes[key]}]));
      changed.forEach(key=>{ctrl(key).value=changes[key];});
    }
    updateAlbumPath();
    const summary=[];
    if(changed.length)summary.push('已更新 '+changed.length+' 项');
    if(verified.length)summary.push(verified.length+' 项已符合目录写法或日期格式');
    if(Object.keys(review).length)summary.push(Object.keys(review).length+' 项待核对');
    sessionResult().innerHTML='<p>'+escape(summary.length?summary.join('，')+'。':Object.values(values).some(Boolean)?'暂无可自动更新的场次字段。':'请先填写场次信息。')+'</p>'+
      (changed.length||Object.keys(review).length?'<ul>'+changed.map(key=>'<li>'+escape(sessions[key]+'：'+values[key]+' → '+changes[key])+'</li>').join('')+
        Object.entries(review).map(([key,note])=>'<li>'+escape(sessions[key]+'：'+note)+'</li>').join('')+'</ul>':'')+
      (previousSession?btn('撤销上次修改','session-undo','quiet'):'');
  }
  function reviewSessionWithModel(){
    if(!desktop?.llmRecognize){normalizeSessionFields();return;}
    const values=sessionValues();
    if(!Object.entries(values).some(([key,value])=>key!=='date'&&value)){sessionResult().textContent='请先填写场次信息。';return;}
    const dialog=document.createElement('dialog');
    dialog.className='program-dialog';
    dialog.setAttribute('aria-label','大模型转英文并核对');
    dialog.innerHTML='<div class="dialog-head"><h2>大模型转英文并核对</h2>'+btn('关闭','dismiss','quiet')+'</div>'+
      '<div class="dialog-body"><p class="hint" data-session-provider>正在读取模型配置…</p>'+
      '<p class="hint">请按节目单核对专名和地点。公开演出资料可在「识别场次信息」中联网核对。</p>'+
      '<p class="hint" role="status" data-status></p><div class="table-wrap" data-review-rows></div></div>'+
      '<div class="dialog-foot">'+btn('使用本机词典','local')+btn('开始核对','run','primary')+btn('应用选中项','apply','primary')+'</div>';
    const $=selector=>query(dialog,selector);
    const status=$('[data-status]'),results=$('[data-review-rows]');
    const run=$('[data-split="run"]'),apply=$('[data-split="apply"]');
    let config,suggestions=[];
    run.disabled=apply.disabled=true;
    $('[data-split="dismiss"]').onclick=()=>dialog.close();
    $('[data-split="local"]').onclick=()=>{dialog.close();normalizeSessionFields();};
    run.onclick=async()=>{
      run.disabled=apply.disabled=true;
      suggestions=[];results.replaceChildren();
      status.textContent='正在请模型核对场次字段…';
      try{
        const response=await llmRecognize('session-english',JSON.stringify(Object.fromEntries(Object.entries(values).filter(([,value])=>value))),[],config);
        if(!dialog.open)return;
        suggestions=modelSessionReview(response,values);
        results.innerHTML=suggestions.length?'<table><thead><tr><th>应用</th><th>字段</th><th>原值</th><th>英文建议</th><th>核对说明</th></tr></thead><tbody>'+
          suggestions.map(({key,before,after,note,needsReview},index)=>'<tr><td>'+
            (after&&after!==before?'<input type="checkbox" value="'+index+'" aria-label="应用'+escape(sessions[key])+'"'+(needsReview?'':' checked')+'>':'')+
            '</td><td>'+escape(sessions[key])+'</td><td>'+escape(before)+'</td><td>'+escape(after||'—')+'</td><td>'+escape(note||'—')+'</td></tr>').join('')+'</tbody></table>':'';
        apply.disabled=!results.querySelector('input:checked');
        status.textContent=suggestions.length?'核对后勾选要写回的项目。':'模型没有给出可用的校对结果。';
      }catch(error){if(dialog.open)status.textContent=error.message;}
      finally{if(dialog.open)run.disabled=false;}
    };
    results.onchange=()=>{apply.disabled=!results.querySelector('input:checked');};
    apply.onclick=()=>{
      const selected=[...results.querySelectorAll('input:checked')].map(input=>suggestions[Number(input.value)]);
      const applicable=selected.filter(item=>ctrl(item.key).value.trim()===item.before);
      if(!applicable.length){status.textContent='选中字段已更改，请关闭窗口后重新核对。';return;}
      previousSession=Object.fromEntries(applicable.map(({key,before,after})=>[key,{before,after}]));
      for(const {key,after} of applicable)ctrl(key).value=after;
      updateAlbumPath();
      sessionResult().innerHTML='<p>已应用 '+applicable.length+' 项模型建议。'+(applicable.length<selected.length?' 部分字段已更改，请重新核对。':'')+'</p>'+
        btn('撤销上次修改','session-undo','quiet');
      dialog.close();
    };
    dialog.addEventListener('close',()=>dialog.remove(),{once:true});
    document.body.append(dialog);dialog.showModal();
    void llmConfig().then(value=>{
      if(!dialog.open)return;
      const ready=value.model&&(value.provider!=='deepseek'||value.hasKey);
      if(!ready){status.textContent='请在「LLM 配置」保存模型和 API Key。';return;}
      config=value;
      $('[data-session-provider]').textContent='点击「开始核对」会把已填写的场次字段发送给 '+value.model+'（'+new URL(value.base).host+'）。';
      run.disabled=false;
    }).catch(error=>{if(dialog.open)status.textContent=error.message;});
  }
  function undoSessionFields(){
    for(const [key,{before,after}] of Object.entries(previousSession||{})){
      const input=ctrl(key);
      if(input.value===after)input.value=before;
    }
    previousSession=null;
    updateAlbumPath();
    sessionResult().textContent='已撤销上次自动修改。';
  }
  function importProgram({session,tracks,overwrite,replace}){
    let updatedFields=0;
    for(const [key,value] of Object.entries(session)){
      const input=ctrl(key);
      const autoName=key==='name'&&state.file&&input?.value===state.file.name.replace(/\.[^.]+$/,'');
      const defaultOrigin=key==='origin'&&!existing&&input?.value==='现场自录';
      if(input&&(overwrite||!input.value.trim()||autoName||defaultOrigin)&&input.value!==value){
        input.value=value;
        updatedFields++;
      }
    }
    updateAlbumPath();
    if(tracks.length){
      const old=state.tracks.filter(t=>t.tags.Title||t.filename||t.end||t.start&&t.start!=='00:00'||t.tags.Composer);
      const imported=tracks.map(item=>{
        const track=firstTrack(item.start);
        const start=timeSeconds(item.start),end=timeSeconds(item.end);
        track.start=Number.isFinite(start)?item.start:'';
        track.end=Number.isFinite(end)&&(start===null||end>start)?item.end:'';
        track.awaitingCue=!track.start;
        track.source=state.file?.name||'';
        track.tags.Title=item.title;
        track.tags.Composer=item.composer;
        return track;
      });
      state.tracks=[...(replace?[]:old),...imported];
      rows();
    }
    if(tracks.length)message('已导入 '+tracks.length+' 首曲目。请核对曲目与分轨时间。');
    if(!tracks.length)sessionResult().textContent=updatedFields?'已填入 '+updatedFields+' 项场次信息。':'已保留现有场次信息。';
  }
  function importCd({tracks,album,files,cover,backCover}){
    if(new Set(files.map(file=>file.name.toLowerCase())).size!==files.length)throw Error('所选音频含有同名文件，请先重命名后重新选择。');
    if(mergedFile&&files.some(file=>file.name.toLowerCase()===mergedFile.name.toLowerCase()))throw Error('导入文件与当前拼接文件同名。');
    state.tracks=tracks.map(row=>{
      const track=firstTrack(row.start);
      return {...track,source:row.source,end:row.end,filename:row.filename,tags:{...track.tags,...row.tags}};
    });
    addInputs(files);
    const first=findFile(tracks[0].source);
    if(first&&first!==state.file)activateRecording(first);
    for(const [fieldName,value] of Object.entries({name:album.title,performer:album.artist,composer:album.composer,'album-date':album.date})){
      if(value)ctrl(fieldName).value=value;
    }
    ctrl('origin').value='CD Rip';
    if(cover){
      query(root,'[data-picker="cover"]').value='';
      applyCover(cover);
    }
    if(backCover){
      query(root,'[data-picker="back-cover"]').value='';
      applyBackCover(backCover);
    }
    updateAlbumPath();
    const imported=['专辑标签',cover&&'封面',backCover&&'封底'].filter(Boolean);
    query(root,'[data-batch-status]').textContent='已导入 '+tracks.length+' 轨音频，以及'+imported.join('、')+'。';
    rows();
  }
  async function seek(start){
    if(!state.file)throw Error('请先导入音频。');
    if(start===null||!Number.isFinite(start))throw Error('请核对试听起点。');
    if(state.native&&audio.readyState&&Number.isFinite(audio.duration)){
      if(start>=audio.duration)throw Error('试听起点超出音频时长。');
      audio.currentTime=start;await audio.play().catch(()=>{});return;
    }
    await listen(start);
  }
  async function jump(delta){
    if(!audio.readyState||!Number.isFinite(audio.duration))return;
    const next=Math.max(0,position()+delta);
    const playing=!audio.paused;
    if(state.native||next>=state.offset&&next<=state.offset+audio.duration){
      audio.currentTime=Math.min(audio.duration,next-state.offset);
      updateClock();
      return;
    }
    const target=recordingDuration===null?next:Math.min(next,Math.max(0,recordingDuration-0.15));
    if(target>=state.offset&&target<=state.offset+audio.duration){
      audio.currentTime=target-state.offset;
      updateClock();
      return;
    }
    await listen(target,playing);
  }
  async function listen(start,play=true){
    if(!state.file)throw Error('请先导入音频。');
    if(!Number.isFinite(start))throw Error('请核对试听起点。');
    state.busy=true;state.stop=false;controls();updateProgress('准备试听片段…',0);
    try{
      const ff=engine();
      const info=await ff.send('prepare',desktop?{id:state.file.id}:{file:state.file});
      recordingDuration=info.duration;
      if(info.duration&&start>=info.duration)throw Error('试听起点超出音频时长。');
      const {bytes}=await ff.send('preview',desktop?{id:state.file.id,start}:start);
      useAudio(new Blob([bytes],{type:'audio/mpeg'}),start,false,play);
      message('试听片段从 '+stamp(start)+' 开始。');
    }finally{state.busy=false;controls();}
  }
  function setTrack(t,key,value){t[key]=value;const row=query(tbody,'[data-row="'+t.id+'"]');query(row,'[data-track="'+key+'"]').value=value;}
  function movementsDialog(track){
    const title=track.tags.Title?.trim();
    if(!title)throw Error('请先填写整部作品或组曲名称。');
    const credit=track.tags.Composer?.trim()||ctrl('composer').value.trim();
    let work=null,request,suggestedParts='';
    const dialog=document.createElement('dialog');
    dialog.setAttribute('aria-label','展开乐章或分曲');
    dialog.innerHTML='<form><div class="dialog-head"><h2>展开乐章／分曲</h2>'+btn('关闭','dismiss','quiet')+'</div><div class="dialog-body">'+
      '<div class="movement-search">'+field('movement-title','作品名称或编号',title)+field('movement-composer','作曲家',credit)+btn('查找乐章','search-parts','primary')+'</div>'+
      (desktop?.llmRecognize?'<div class="actions section-gap">'+btn('用模型匹配乐章','model-parts')+'<span class="hint" role="status" data-model-status></span></div>':'')+
      '<p class="hint" role="status" data-movement-status></p><div data-movement-choices></div><div data-movement-result></div>'+
      '<details data-manual-movements><summary>手动输入或核对乐章</summary><label class="field"><span>按演出顺序，每行一项</span><textarea name="manual-parts" rows="6" placeholder="Allegro&#10;Andante&#10;Finale"></textarea></label><button class="button" type="submit" data-apply-manual>填入手动输入的乐章</button></details>'+
      '<p class="hint">选中的乐章将按顺序替换当前曲目。第一轨保留原起点，最后一轨保留原终点，中间时间留待标记。</p><div class="form-error" role="alert"></div></div>'+
      '<div class="dialog-foot" data-movement-footer hidden><button class="button primary" type="submit" data-apply-directory>按顺序填入分轨表</button></div></form>';
    document.body.append(dialog);
    const status=query(dialog,'[data-movement-status]');
    const choices=query(dialog,'[data-movement-choices]');
    const result=query(dialog,'[data-movement-result]');
    const manual=query(dialog,'[data-manual-movements]');
    const editor=query(manual,'textarea');
    const input=query(dialog,'[name="movement-title"]');
    const composerInput=query(dialog,'[name="movement-composer"]');
    const error=query(dialog,'.form-error');
    const footer=query(dialog,'[data-movement-footer]');
    const applyDirectory=query(footer,'button');
    const sourceLink=url=>'<a class="work-source" href="'+escape(url)+'" target="_blank" rel="noopener noreferrer">核对乐章目录 ↗</a>';
    function selectedParts(){
      return [...result.querySelectorAll('input[name="part"]:checked')].map(input=>work.entries[Number(input.value)]);
    }
    function updateSelection(){
      const parts=selectedParts();
      const preview=query(result,'[data-selected-parts]');
      preview.innerHTML=parts.length?'<ol class="movement-list">'+parts.map(({name,index,number,numbered})=>
        '<li>'+escape(numbered?name:'目录第 '+(number||index+1)+' 项 · '+name)+'</li>').join('')+'</ol>':'';
      query(result,'[data-selection-count]').textContent='已选 '+parts.length+' / '+work.entries.length+' 项';
      applyDirectory.disabled=!parts.length;
      error.textContent='';
    }
    function resetWork(){
      work=null;
      footer.hidden=true;
      result.replaceChildren();
      if(editor.value===suggestedParts)editor.value='';
      suggestedParts='';
    }
    function showWork(candidate,ordered=true){
      work={...candidate};
      result.innerHTML='<strong>'+escape(work.title)+'</strong><p class="hint">'+escape(work.composer)+'</p>'+(work.source?sourceLink(work.source):'');
      if(ordered){
        const fromSearch=!!work.entries;
        work.entries??=work.parts.map((name,index)=>({name,index,numbered:work.numberedTitles}));
        const partList=(entries,offset)=>'<ol class="movement-list">'+entries.map(({name,number,source},i)=>{
          const index=offset+i;
          const checked=!fromSearch&&(!work.mainCount||index<work.mainCount);
          return '<li><label><input type="checkbox" name="part" value="'+index+'"'+(checked?' checked':'')+'> '+escape((number?number+'. ':'')+name)+'</label>'+
            (source?' <a class="work-source" href="'+escape(source)+'" target="_blank" rel="noopener noreferrer">来源 ↗</a>':'')+'</li>';
        }).join('')+'</ol>';
        const mainCount=work.mainCount||work.entries.length;
        result.insertAdjacentHTML('beforeend','<div class="actions movement-picker-tools">'+btn('全选','pick-all')+btn('清空选择','pick-none')+'<span class="hint" role="status" data-selection-count></span></div>'+
          partList(work.entries.slice(0,mainCount),0)+
          (work.mainCount?'<strong>附录 · 遗作变奏</strong><p class="hint">附录按编号列在正文之后。若演出将遗作变奏穿插在正文中，可在下方按演出顺序手动填入。</p>'+partList(work.entries.slice(mainCount),mainCount):'')+
          '<details class="movement-selection"><summary>预览填入顺序</summary><div data-selected-parts></div></details>');
        status.textContent=work.fromModel?'请核对作品版本与乐章顺序，勾选要填入的条目。':fromSearch?'可直接勾选搜索结果中的乐章。当前结果可能只包含部分乐章。':'勾选要填入的乐章。分轨表按目录顺序排列，并保留乐章原编号。';
        footer.hidden=false;
        updateSelection();
        return;
      }
      manual.open=true;
      status.textContent=work.parts.length?'目录顺序未确认或条目可能缺失。请在下方核对、补齐并排序。':'该作品条目未收录乐章。可换一个作品版本，或粘贴节目单上的乐章。';
      if(!editor.value.trim()){
        suggestedParts=work.parts.filter(Boolean).join('\n');
        editor.value=suggestedParts;
      }
    }
    async function fetchWorks(params,signal){
      const response=await fetch('/api/work-search?'+new URLSearchParams(params),{signal});
      const data=await response.json();
      if(!response.ok)throw Error(data.error);
      return data;
    }
    async function selectWork(candidate){
      request?.abort();
      const controller=new AbortController();
      request=controller;
      resetWork();
      status.textContent='正在读取乐章顺序…';
      try{
        const data=await fetchWorks({id:candidate.id},controller.signal);
        if(controller.signal.aborted)return;
        showWork({title:candidate.title,composer:data.composer||candidate.composer,source:candidate.url,parts:data.parts},data.ordered);
      }catch(err){
        if(!controller.signal.aborted){status.textContent=err.message||'乐章目录读取失败。';manual.open=true;}
      }
    }
    async function lookup(){
      request?.abort();
      const controller=new AbortController();
      request=controller;
      const raw=input.value.trim();
      const composer=composerInput.value.trim();
      resetWork();
      choices.replaceChildren();
      error.textContent='';
      if(!raw){status.textContent='请输入作品名称或编号。';return;}
      const local=findMovementList(raw,composer);
      if(local){showWork(local);return;}
      status.textContent='正在查找 MusicBrainz 作品目录…';
      try{
        const data=await fetchWorks({...movementSearch(raw,composer),mode:'parts'},controller.signal);
        if(controller.signal.aborted)return;
        if(!data.works.length){
          status.textContent='未找到作品。可试试英文曲名、作曲家或作品编号，也可手动输入乐章。';
          manual.open=true;
          return;
        }
        const direct=searchResultMovements(data.works);
        const parents=data.works.filter(candidate=>!direct.some(group=>group.entries.some(entry=>entry.source===candidate.url)));
        for(const candidate of parents){
          const button=document.createElement('button');
          button.type='button';
          button.className='button work-choice';
          button.textContent=[candidate.title,candidate.composer,candidate.note].filter(Boolean).join(' · ');
          button.onclick=()=>void selectWork(candidate);
          choices.append(button);
        }
        if(direct.length){
          for(const group of direct.length>1||parents.length?direct:[]){
            const button=document.createElement('button');
            button.type='button';
            button.className='button work-choice';
            button.textContent=group.title+' · '+group.composer+'（搜索中的乐章）';
            button.onclick=()=>{request?.abort();resetWork();showWork(group);};
            choices.prepend(button);
          }
          showWork(direct[0]);
          return;
        }
        if(parents.length===1){void selectWork(parents[0]);return;}
        status.textContent='请选择作品，核对版本与作品编号后可查看乐章。';
      }catch(err){
        if(!controller.signal.aborted){status.textContent=err.message||'在线查询失败。';manual.open=true;}
      }
    }
    dialog.addEventListener('close',()=>{request?.abort();dialog.remove();},{once:true});
    query(dialog,'[data-split="dismiss"]').onclick=()=>dialog.close();
    query(dialog,'[data-split="search-parts"]').onclick=()=>void lookup();
    const ai=query(dialog,'[data-split="model-parts"]');
    if(ai)attachWorkModel(ai,query(dialog,'[data-model-status]'),()=>({title:input.value.trim(),composer:composerInput.value.trim()}),'movements',result=>{
      const candidate=modelMovementList(result);
      if(!candidate)return '模型未能确定乐章。可补充作品编号或指定版本后重试。';
      request?.abort();
      resetWork();
      showWork(candidate);
      return '已列出 '+candidate.entries.length+' 项模型建议，顺序和版本需核对。';
    });
    result.addEventListener('change',event=>{
      if(event.target.name==='part')updateSelection();
    });
    result.addEventListener('click',event=>{
      const action=event.target.closest('button')?.dataset.split;
      if(action!=='pick-all'&&action!=='pick-none')return;
      for(const checkbox of result.querySelectorAll('input[name="part"]'))checkbox.checked=action==='pick-all';
      updateSelection();
    });
    for(const field of [input,composerInput])field.addEventListener('keydown',event=>{
      if(event.key==='Enter'&&!event.isComposing){event.preventDefault();void lookup();}
    });
    dialog.addEventListener('submit',event=>{
      event.preventDefault();
      const parts=event.submitter?.hasAttribute('data-apply-manual')
        ? editor.value.trim().split(/\r?\n/u).map(line=>line.trim().replace(/^(?:\d+|[IVXLCDM]+)[.)、]\s*/u,'')).filter(Boolean).map((name,index)=>({name,index}))
        : selectedParts();
      if(!parts.length){error.textContent='请至少选择或填写一项。';return;}
      const titles=movementTitles(work?.title||title,parts);
      if(titles.some(name=>name.length>280)){error.textContent='有分曲标题超过 280 个字符，请缩短后再填入。';return;}
      const index=state.tracks.indexOf(track);
      if(index<0)return;
      const children=expandMovementTrack(track,titles,work?.composer||composerInput.value.trim());
      state.tracks.splice(index,1,...children);
      query(root,'[data-batch-status]').textContent='已按顺序填入 '+children.length+' 项乐章或分曲。请标记分轨时间。';
      rows();
      dialog.close();
      query(tbody,'[data-row="'+children[0].id+'"] [data-track="Title"]').focus();
    });
    dialog.showModal();
    void lookup();
  }
  function tagDialog(t){
    const dialog=document.createElement('dialog');
    dialog.innerHTML='<form><div class="dialog-head"><h2>编辑曲目标签</h2>'+btn('关闭','dismiss','quiet')+'</div><div class="dialog-body"><div class="form-grid">'+field('filename','英文文件名（可留空）',t.filename)+tags.map(k=>field(k,k,t.tags[k])).join('')+'</div><p class="hint">留空的标签会从场次信息与曲目顺序自动补齐。</p></div><div class="dialog-foot"><button class="button primary">保存曲目标签</button></div></form>';
    dialog.setAttribute('aria-label','编辑曲目标签');
    document.body.append(dialog);
    dialog.addEventListener('close',()=>dialog.remove(),{once:true});
    query(dialog,'[data-split="dismiss"]').onclick=()=>dialog.close();
    dialog.addEventListener('submit',e=>{
      e.preventDefault();const f=new FormData(e.target);
      t.filename=f.get('filename');for(const k of tags)t.tags[k]=f.get(k);
      rows();dialog.close();
    });
    dialog.showModal();
  }
  let lastTagUpdate;
  function tagUpdateStatus(text){
    const area=query(root,'[data-tag-sync]');
    area.innerHTML='<span class="hint" role="status">'+escape(text)+'</span>'+
      (lastTagUpdate?btn('撤销批量标签更新','undo-batch-tags','quiet'):'');
  }
  function updateTrackTags(){
    const {updates,matched,hasSession}=batchSessionTags({...sessionValues(),albumDate:ctrl('album-date').value.trim()},state.tracks);
    if(!hasSession){tagUpdateStatus('请先填写可用的专辑名、演奏家、日期等场次信息。');return;}
    if(!matched){tagUpdateStatus('请先添加曲目或为音轨选择来源。');return;}
    if(!updates.size){tagUpdateStatus('各轨标签已符合当前场次信息。');return;}
    for(const track of state.tracks){
      for(const {key,after} of updates.get(track.id)||[])track.tags[key]=after;
    }
    lastTagUpdate=updates;
    rows();
    tagUpdateStatus('已更新 '+updates.size+' 轨的标签。下次导出将写入这些值。');
  }
  function undoTrackTagUpdate(){
    if(!lastTagUpdate)return;
    for(const track of state.tracks){
      for(const {key,before,after} of lastTagUpdate.get(track.id)||[]){
        if(track.tags[key]!==after)continue;
        if(before===undefined)delete track.tags[key];
        else track.tags[key]=before;
      }
    }
    lastTagUpdate=null;
    rows();
    tagUpdateStatus('已撤销上次批量更新。');
  }
  function matchAll(){
    const extension=outputExtension();
    const {names,review,preserved,replaced}=batchTrackFilenames(state.tracks,{composer:ctrl('composer').value.trim(),extension});
    for(const track of state.tracks){
      if(names.has(track.id))track.filename=names.get(track.id);
    }
    rows();
    const summary=[];
    if(names.size)summary.push('已生成 '+names.size+' 个文件名（.'+extension+'）');
    if(replaced)summary.push('已替换 '+replaced+' 个已有文件名');
    if(preserved)summary.push('保留 '+preserved+' 个已有文件名');
    if(review.length)summary.push('请逐轨核对第 '+review.join('、')+' 轨');
    query(root,'[data-batch-status]').textContent=summary.length?summary.join('。')+'。':'请先填写曲目名称。';
  }
  function matchDialog(track){
    const dialog=document.createElement('dialog');
    dialog.setAttribute('aria-label','匹配规范文件名');
    dialog.innerHTML='<div class="dialog-head"><h2>匹配规范文件名</h2>'+btn('关闭','dismiss','quiet')+'</div>'+
      '<div class="dialog-body"><form class="work-search-form">'+field('work','曲目名、简称或作品编号',track.tags.Title)+'<button class="button primary" type="submit">查找</button></form>'+
      '<p class="hint">例如「肖协2 第三乐章」「K331」「柴六」。打开时自动匹配站内曲目并尝试补充 MusicBrainz 候选（仅发送作品检索词与作曲家）。</p>'+
      (desktop?.llmRecognize?'<div class="actions section-gap">'+btn('用模型匹配文件名','model-filename')+'<span class="hint" role="status" data-model-status></span></div>':'')+
      '<label><input type="checkbox" name="update-tags"> 同时更新 Title 和 Composer 标签</label><p class="hint" role="status" data-match-status></p><div data-matches></div></div>';
    document.body.append(dialog);
    const search=query(dialog,'[name="work"]');
    const status=query(dialog,'[data-match-status]');
    const list=query(dialog,'[data-matches]');
    let request,timer;
    dialog.addEventListener('close',()=>{clearTimeout(timer);request?.abort();dialog.remove();root.querySelector('[data-split="match"][data-key="'+track.id+'"]')?.focus();},{once:true});
    query(dialog,'[data-split="dismiss"]').onclick=()=>dialog.close();
    const index=state.tracks.indexOf(track);
    const extension=outputExtension();
    const stem=(candidate,style)=>{
      const label=style==='composer'&&candidate.composer?candidate.composer+' - '+candidate.title:candidate.title;
      return filenameFor('{Track} - {Title}',{...track,filename:''},{Track:track.tags.Track?.trim()||String(index+1).padStart(2,'0'),Title:label},index);
    };
    function show(candidates){
      list.replaceChildren();
      for(const candidate of candidates){
        const card=document.createElement('article');
        card.className='work-candidate';
        card.innerHTML='<strong>'+escape(candidate.title)+'</strong><p class="hint">'+escape([candidate.composer,candidate.source].filter(Boolean).join(' · '))+'</p>';
        if(candidate.url){
          const ref=document.createElement('a');
          ref.className='work-source';
          ref.href=candidate.url;ref.target='_blank';ref.rel='noopener noreferrer';ref.textContent='查看作品来源 ↗';
          card.append(ref);
        }
        for(const style of candidate.composer?['title','composer']:['title']){
          const name=stem(candidate,style);
          if(!name||!outputFilenameValid(name+'.'+extension))continue;
          const choice=document.createElement('button');
          choice.type='button';choice.className='button work-choice';
          choice.textContent=name+'.'+extension;
          choice.onclick=()=>{
            track.filename=name;
            if(query(dialog,'[name="update-tags"]').checked){
              track.tags.Title=candidate.title;
              if(candidate.composer)track.tags.Composer=candidate.composer;
            }
            rows();dialog.close();
          };
          card.append(choice);
        }
        if(card.querySelector('button'))list.append(card);
      }
    }
    async function lookup(online=false){
      request?.abort();
      const raw=search.value.trim();
      const composer=getQuery().composer;
      const q=parseWorkQuery(raw,composer);
      if(raw.length<2){show([]);status.textContent='请输入曲目名、简称或作品编号。';return;}
      const local=matchWorks(q,getProjects());
      show(mergeWorks(local,[],q));
      status.textContent=list.children.length?'请选择要用的文件名。':'站内未找到合适的候选，可补充作曲家或作品编号。';
      if(!online)return;
      const text=q.online||q.base;
      if(text.length<2&&list.children.length)return;
      if(!text){status.textContent='请补充作品名称或编号，再查找在线候选。';return;}
      const controller=new AbortController();
      request=controller;
      status.textContent='正在查找 MusicBrainz…';
      try{
        const response=await fetch('/api/work-search?'+new URLSearchParams({work:text,artist:q.artist||composer}),{signal:controller.signal,cache:'no-store'});
        const result=await response.json();
        if(controller.signal.aborted)return;
        if(!response.ok)throw Error(result.error);
        show(mergeWorks(local,result.works,q));
        status.textContent=list.children.length?'请选择要用的文件名。':'没有找到合适的候选，可试试英文作品名或更完整的作品编号。';
      }catch(error){if(!controller.signal.aborted)status.textContent=error.message||'在线查询失败，请稍后重试。';}
    }
    const getQuery=()=>({title:search.value.trim(),composer:track.tags.Composer?.trim()||ctrl('composer').value.trim()});
    const ai=query(dialog,'[data-split="model-filename"]');
    if(ai)attachWorkModel(ai,query(dialog,'[data-model-status]'),getQuery,'filenames',result=>{
      if(!Array.isArray(result)||result.length>5)throw Error('模型没有返回有效的曲目候选。');
      const seen=new Set();
      const candidates=result.flatMap(item=>{
        const title=typeof item?.title==='string'?item.title.trim().replace(/\.(?:flac|wav|mp3|mkv|ape)$/iu,''):'';
        const composer=typeof item?.composer==='string'?item.composer.trim():'';
        const id=(title+' '+composer).normalize('NFKC').toLowerCase();
        if(!title||title.length>200||composer.length>160||/[\x00-\x1f]/u.test(title+composer)||seen.has(id))return [];
        seen.add(id);
        return [{title,composer,source:'模型建议 · 待核对'}];
      });
      if(!candidates.length)return '模型未能确定曲目。可补充作品号或作曲家后重试。';
      clearTimeout(timer);
      request?.abort();
      show(candidates);
      status.textContent=list.children.length?'请选择文件名。勾选标签选项后可同时更新标签。':'建议的文件名过长，请补充更具体的检索词。';
      return '已收到 '+candidates.length+' 项模型建议。';
    });
    query(dialog,'form').addEventListener('submit',event=>{event.preventDefault();clearTimeout(timer);void lookup(true);});
    search.addEventListener('input',()=>{clearTimeout(timer);request?.abort();status.textContent='';list.replaceChildren();timer=setTimeout(()=>void lookup(),160);});
    dialog.showModal();
    void lookup(true);
  }
  async function clearCache(){
    if(state.cache&&state.cacheName)try{await state.cache.removeEntry(state.cacheName,{recursive:true});}catch{}
    state.cache=null;state.cacheName='';
  }
  async function keepOutput(bytes,job,format){
    const blob=new Blob([bytes],{type:audioTypes[format]});
    if(state.folder){
      let handle;
      try{
        try{
          await state.folder.getFileHandle(job.target);
          return {blob,note:'输出目录已有同名文件。可单独下载此轨。'};
        }catch(e){if(e.name!=='NotFoundError')throw e;}
        handle=await state.folder.getFileHandle(job.target,{create:true});
        if((await handle.getFile()).size>0)return {blob,note:'输出目录已有同名文件。可单独下载此轨。'};
        const stream=await handle.createWritable({mode:'exclusive'});
        try{await stream.write(blob);await stream.close();}catch(e){try{await stream.abort();}catch{}throw e;}
        return {handle,note:'已保存到 '+state.folder.name};
      }catch(e){return {blob,note:'写入目录失败，可单独下载。'+e.message};}
    }
    if(state.cache){
      try{
        const dir=await state.cache.getDirectoryHandle(state.cacheName,{create:true});
        const handle=await dir.getFileHandle(job.target,{create:true});
        const stream=await handle.createWritable();
        try{await stream.write(blob);await stream.close();}catch(e){try{await stream.abort();}catch{}throw e;}
        return {handle};
      }catch{}
    }
    return {blob};
  }
  async function run(){
    const limitNotice=query(root,'[data-browser-limit]');
    limitNotice.hidden=true;
    const {project,plan,rule}=collect();
    if(coverRequired&&!state.cover)throw Error('请重新选择项目的封面，或点击「移除封面」后继续。');
    if(desktop&&!state.folder){await chooseFolder();if(!state.folder)return;}
    state.busy=true;state.stop=false;controls();message('');
    let success=0,failed=0,saved=false;
    try{
      updateProgress('正在加载 FFmpeg 和检查录音…',0);
      const ff=engine(),mediaBySource=new Map();
      let preparedSource;
      const artwork=async file=>file?{name:file.name,bytes:new Uint8Array(await file.arrayBuffer())}:null;
      const [cover,backCover]=desktop?await Promise.all([artwork(state.cover),artwork(state.backCover)]):[];
      async function prepareSource(source){
        const file=findFile(source);
        const info=await ff.send('prepare',desktop?{id:file.id}:{file,cover:state.cover});
        preparedSource=source;
        return info;
      }
      for(const job of plan.jobs){
        if(!mediaBySource.has(job.source))mediaBySource.set(job.source,await prepareSource(job.source));
        const info=mediaBySource.get(job.source);
        if(plan.settings.width!==100&&info.channels!==2)throw Error(job.source+' 不是双声道。请将声场宽度设回 100%。');
        if(info.duration&&(job.start>=info.duration||(job.end!==null&&job.end>info.duration+0.05)))throw Error('第 '+job.order+' 轨的时间点超出音频时长 '+stamp(info.duration)+'。');
        const length=(job.end??info.duration)-job.start;
        const projected=length*plan.settings.rate*info.channels*3;
        if(!desktop&&plan.format!=='MP3'&&length>0&&projected>450*1024*1024){
          query(limitNotice,'[data-limit-note]').textContent='第 '+job.order+' 轨预计超出浏览器处理容量（时长 '+stamp(length)+'，按 24 位 PCM 估算 '+Math.ceil(projected/1048576)+' MiB）。可保留整轨，改用本机 PowerShell 处理。';
          limitNotice.hidden=false;
          return;
        }
      }
      if(state.stop)return;
      const outputFolder=desktop?await nativeCall(desktop.prepareAlbumFolder(state.folder.id,plan.output,cover,backCover)):null;
      if(state.stop)return;
      const savedProject=await storeDraft(project,rule);saved=true;
      state.results=[];results();await clearCache();
      if(!desktop&&!state.folder){
        try{state.cache=await navigator.storage?.getDirectory();state.cacheName='classical-audio-'+crypto.randomUUID();}catch{}
      }
      for(const [i,job] of plan.jobs.entries()){
        if(state.stop)break;
        state.trackDuration=(job.end??mediaBySource.get(job.source).duration)-job.start;
        updateProgress('第 '+(i+1)+' / '+plan.jobs.length+' 轨 · '+job.target,0);
        try{
          if(preparedSource!==job.source){preparedSource=null;await prepareSource(job.source);}
          const params={job,settings:plan.settings,format:plan.format};
          if(desktop)Object.assign(params,{id:findFile(job.source).id,folder:outputFolder.id});
          const {bytes,duration,id,note}=await ff.send('track',params);
          const stored=desktop?{nativeId:id,note}:await keepOutput(bytes,job,plan.format);
          state.results.push({job,duration,...stored});success++;
          saveResult(savedProject.id,{job,duration,settings:plan.settings,format:plan.format,note:stored.note});
        }catch(error){
          if(state.stop)break;
          failed++;state.results.push({job,error:error.message});saveResult(savedProject.id,{job,error:error.message});
        }
        results();
      }
      message(state.stop?'已停止。已完成的曲目已写入专辑文件夹。':'完成 '+success+' 轨，失败 '+failed+' 轨。'+(success?(outputFolder?'已保存到 '+outputFolder.name+'。'+(outputFolder.coverName?'封面已一并保存为 '+outputFolder.coverName+'。':'')+(outputFolder.backCoverName?'封底已保存为 '+outputFolder.backCoverName+'。':''):'请在下方下载，或查看选定的输出目录。'):''));
    }finally{
      if(saved)finishProject(state.savedId,{success,failed,stopped:state.stop,cover:!!state.cover});
      state.busy=false;state.trackDuration=0;controls();
    }
  }
  root.addEventListener('input',event=>{
    const input=event.target;
    if(Object.hasOwn(sessions,input.name)||input===albumField)updateAlbumPath();
    if(input.dataset.track){
      const t=state.tracks.find(x=>x.id===input.closest('[data-row]').dataset.row);
      if(input.dataset.track==='Title'){
        t.tags.Title=input.value;
        input.closest('td').querySelector('[data-title-check]')?.remove();
      }
      if(input.dataset.track!=='Title')t[input.dataset.track]=input.value;
      if(input.dataset.track==='filename')query(root,'[data-batch-status]').textContent='';
    }
    
  });
  root.addEventListener('change',async event=>{
    const input=event.target,kind=input.dataset.picker,file=input.files?.[0];
    if(input.dataset.track==='Title'&&/\p{Script=Han}/u.test(input.value)){
      const track=state.tracks.find(t=>t.id===input.closest('[data-row]').dataset.row);
      const match=englishProgramTrack({title:input.value,composer:track.tags.Composer||ctrl('composer').value});
      const note=document.createElement('small');
      note.className='split-filename';
      note.setAttribute('data-title-check','');
      note.setAttribute('role','status');
      if(match.englishOriginal){
        track.tags.Title=match.title;
        track.tags.Composer=match.composer;
        input.value=match.title;
        note.textContent='已匹配英文曲名 · 原文：'+match.englishOriginal;
      }
      if(match.englishReview)note.textContent='待核对英文曲名';
      if(note.textContent)input.after(note);
      return;
    }
    if(input.dataset.track==='source'){
      const chosen=findFile(input.value);
      if(chosen&&chosen!==state.file)activateRecording(chosen);
      query(root,'[data-batch-status]').textContent=chosen?'已切换到 '+chosen.name+'。请核对本轨在该文件中的起止点。':'';
    }
    if(input.name==='preview-source'){
      const chosen=findFile(input.value);
      if(chosen&&chosen!==state.file)activateRecording(chosen);
      if(!chosen)input.value=state.file?.name||'';
    }
    if(!kind)return;
    message('');
    if(kind==='audio'){
      const selected=[...input.files];
      input.value='';
      if(!selected.length)return;
      if(selected.some(item=>!sourceValid(item.name)||!item.size)){message('请选择非空的 WAV、FLAC、MP3 或 MKV 文件。');return;}
      addInputs(selected);
    }
    if(kind==='cover'||kind==='back-cover'){
      const back=kind==='back-cover';
      const apply=back?applyBackCover:applyCover;
      const status=back?text=>{query(root,'[data-back-cover-status]').textContent=text;}:message;
      if(!file){apply(null);return;}
      preparingArtwork=prepareArtwork(file,convertArtwork);
      controls();
      if(/\.hei[cf]$/iu.test(file.name))status('正在转换 '+file.name+'…');
      try{
        const prepared=await preparingArtwork;
        if(!disposed){apply(prepared);if(!back)message('');}
      }catch(error){
        if(!disposed){status(error.message);input.value='';}
      }finally{
        preparingArtwork=null;
        if(!disposed)controls();
      }
    }
  });
  root.addEventListener('click',async event=>{
    const control=event.target.closest('[data-split]');
    if(!control)return;
    const {split,key}=control.dataset;
    const t=state.tracks.find(x=>x.id===key);
    try{
      if(split==='add'||split==='mark-add'){
        if(split==='mark-add'&&(!state.file||!audio.readyState))throw Error('请先试听，再取播放点。');
        const previous=state.tracks.at(-1);
        const source=state.file?.name||previous?.source||'';
        const start=split==='mark-add'?stamp(position()):(previous?.source===source?previous.end:'');
        state.tracks.push({...firstTrack(start),source});rows();
        tbody.lastElementChild.querySelector('[data-track="Title"]').focus();
      }
      if(split==='remove'){state.tracks=state.tracks.filter(x=>x!==t);rows();}
      if(split==='mark-start'||split==='mark-end'){
        if(!state.file)throw Error('请先导入音频。');
        if(t.source&&trackFile(t)!==state.file)throw Error('请先试听此轨选择的音频，再取播放点。');
        if(!audio.readyState)throw Error('请先试听，再取播放点。');
        if(!t.source){t.source=state.file.name;query(tbody,'[data-row="'+t.id+'"] select').value=t.source;}
        setTrack(t,split.slice(5),stamp(position()));
      }
      if(split==='movements')movementsDialog(t);
      if(split==='tags')tagDialog(t);
      if(split==='match')matchDialog(t);
      if(split==='match-all')matchAll();
      if(split==='batch-tags')updateTrackTags();
      if(split==='undo-batch-tags')undoTrackTagUpdate();
      if(split==='session-english')reviewSessionWithModel();
      if(split==='session-undo')undoSessionFields();
      if(split==='program'||split==='recognize-session')openProgramImport({
        fields:sessions,existingCount:state.tracks.filter(t=>t.tags.Title).length,apply:importProgram,sessionOnly:split==='recognize-session',
        initialSession:Object.fromEntries(Object.entries(sessionValues()).filter(([key,value])=>value&&key!=='origin'&&!(key==='name'&&value===state.file?.name.replace(/\.[^.]+$/u,''))))
      });
      if(split==='cd-import')openCdImport({
        chooseFiles:()=>nativeCall(desktop.selectAudio()),
        convertArtwork,
        existingTracks:state.tracks.some(track=>track.tags.Title||track.filename||track.end||track.start&&track.start!=='00:00'),
        apply:importCd
      });
      if(split==='merge')await mergeInputs();
      if((split==='download-merged'||split==='local-merged')&&mergedFile){
        if(desktop)await nativeCall(desktop.saveAudio(mergedFile.id,mergedFile.name));
        else downloadFile(mergedFile);
      }
      if(['input-up','input-down','input-remove'].includes(split)){
        const next=inputs.slice(),index=Number(key);
        const [file]=next.splice(index,1);
        if(split!=='input-remove')next.splice(index+(split==='input-up'?-1:1),0,file);
        updateInputs(next);
      }
      if(split==='llm-cover'){
        if(!state.cover)throw Error('请先上传封面。');
        openProgramImport({fields:sessions,existingCount:state.tracks.filter(t=>t.tags.Title).length,apply:importProgram,modelFiles:[state.cover]});
      }
      if(split==='llm-audio'){
        if(!state.file?.id)throw Error('请先导入并选择录音。');
        const snapshot=state.file;
        const metadata=await nativeCall(desktop.process('metadata',{id:snapshot.id}));
        void openLlmFiles({text:'文件名：'+snapshot.name+'\n'+JSON.stringify(metadata,null,2)});
      }
      if(split==='make-cover'){
        openCoverStudio({
          convertArtwork,
          session:Object.fromEntries(Object.keys(sessions).map(name=>[name,ctrl(name).value.trim()])),
          works:state.tracks.filter(track=>track.tags.Title?.trim()).slice(0,12).map(track=>({title:track.tags.Title,composer:track.tags.Composer})),
          apply:file=>{query(root,'[data-picker="cover"]').value='';applyCover(file);},
          download:downloadFile
        });
      }
      if(split==='back-five'||split==='forward-five')await jump(split==='back-five'?-5:5);
      if(split==='native'&&state.file){
        const previous=position();
        useAudio(state.file);
        audio.addEventListener('loadedmetadata',()=>{if(previous<audio.duration)audio.currentTime=previous;},{once:true});
      }
      if(split==='remove-back-cover'){
        query(root,'[data-picker="back-cover"]').value='';
        applyBackCover(null);
      }
      if(split==='remove-cover'){
        coverRequired=false;applyCover(null);
        query(root,'[data-picker="cover"]').value='';
      }
      if(split==='seek'){
        const file=trackFile(t);
        if(!file)throw Error('请先选择或重新导入此轨的录音。');
        if(file!==state.file)activateRecording(file);
        await seek(timeSeconds(t.start.trim())??0);
      }
      if(split==='preview'){const panel=query(root,'.split-preview');panel.hidden=!panel.hidden;if(!panel.hidden)ctrl('preview-time').value=stamp(position());}
      if(split==='listen')await listen(timeSeconds(ctrl('preview-time').value.trim())??0);
      if(split==='folder')await chooseFolder();
      if(split==='select-audio')addInputs(await nativeCall(desktop.selectAudio()));
      if(split==='relocate-audio'){
        const ref=missingAudio[Number(key)];
        if(ref)addInputs(await nativeCall(desktop.selectAudio(ref.name)));
      }
      if(split==='reveal')await nativeCall(desktop.reveal(state.results[Number(key)].nativeId));
      if(split==='new-session'){
        await root.dispose();
        newSession();
      }
      if(split==='cover-videos')coverVideoDialog();
      if(split==='preview-effects')previewEffectsDialog();
      if(split==='run')await run();
      if(split==='local-script'){
        const {project,plan,rule}=collect(true,false);
        const savedProject=await storeDraft(project,rule);
        controls();
        openLocalScript(savedProject,plan);
      }
      if(split==='local-cover'&&state.cover)downloadFile(state.cover);
      if(split==='stop'){state.stop=true;(desktop?nativeEngine:state.engine)?.close();}
      if(split==='save'){
        const {project,rule}=collect(false);
        await storeDraft(project,rule);controls();message('已保存为录音项目。音频与封面文件保留在本机。');
      }
      if(split==='open')openProject(state.savedId);
      if(split==='download'){
        const r=state.results[Number(key)];
        const blob=r.blob||await r.handle.getFile();
        const link=document.createElement('a');
        link.href=URL.createObjectURL(blob);link.download=r.job.target;link.click();
        setTimeout(()=>URL.revokeObjectURL(link.href),30000);
      }
    }catch(error){if(error.name!=='AbortError')message(state.stop?'已停止处理。':error.message);}
  });
  function draftSnapshot(){
    const settings=[...root.querySelectorAll('[name="format"],[name="rate"],[name="gain"],[name="width"],[name="reverb"],[name^="effect-"]')]
      .map(input=>input.type==='checkbox'?input.checked:input.value);
    const sources=[...new Set([...state.project.sources,...loadedFiles().map(file=>file.name)])];
    return JSON.stringify({session:sessionValues(),albumDate:ctrl('album-date').value.trim(),settings,tracks:state.tracks,sources,recordingParts:activeInputs.map(file=>file.name),
      mediaAudio:inputs.map(file=>file.id||file.name),missingAudio:missingAudio.map(file=>file.id),mergedId:mergedFile?.id||'',cover:state.cover?.name||'',coverSize:state.cover?.size||0,coverRequired,backCover:state.backCoverFile,backCoverSize:state.backCover?.size||0,output:albumField.value.trim(),rule:ctrl('rule').value.trim(),join:ctrl('join').checked});
  }
  function storeDraft(project,rule){
    if(mediaSave)return mediaSave;
    mediaSave=(async()=>{
      if(desktop?.rememberProjectImage){
        if(inputs.length+missingAudio.length>500)throw Error('单个项目最多保存 500 条原始音频引用。');
        for(const role of ['cover','backCover']){
          const file=state[role];
          if(file&&!imageIds[role])imageIds[role]=await nativeCall(desktop.rememberProjectImage({
            name:file.name,bytes:new Uint8Array(await file.arrayBuffer())
          }));
        }
        const record=file=>({id:file.id,name:file.name});
        project.localMedia={audio:[...inputs.map(record),...missingAudio],
          merged:mergedFile?record(mergedFile):missingMerged,cover:imageIds.cover,backCover:imageIds.backCover};
      }
      const saved=saveProject(project,rule);
      state.savedId=saved.id;
      savedDraft=draftSnapshot();
      return saved;
    })().finally(()=>{mediaSave=null;controls();});
    controls();
    return mediaSave;
  }
  function renderMissingMedia(){
    const list=query(root,'[data-missing-media]');
    if(!list)return;
    list.hidden=!missingAudio.length&&!missingMerged;
    list.innerHTML=missingAudio.map((ref,index)=>'<li><span>'+escape(ref.name)+' · 找不到文件</span>'+
      btn('重新关联','relocate-audio','quiet',String(index))+'</li>').join('')+
      (missingMerged?'<li>'+escape(missingMerged.name)+' · 拼接文件缺失，可用原片段重新拼接。</li>':'');
  }
  async function restoreProjectMedia(){
    if(!desktop?.restoreProjectMedia||!refs)return;
    restoring=true;controls();
    const status=query(root,'[data-media-status]');
    status.textContent='正在恢复本地音频与图片…';
    missingAudio=refs.audio;
    try{
      const media=await nativeCall(desktop.restoreProjectMedia({audio:refs.audio.map(item=>item.id),
        merged:refs.merged?.id,cover:refs.cover,backCover:refs.backCover}));
      if(disposed)return;
      missingAudio=refs.audio.filter((_,index)=>!media.audio[index]);
      if(media.merged){
        missingMerged=null;
        mergedFile=media.merged;
        mergedParts=savedParts.slice();
        activeInputs=media.audio.filter(file=>file&&savedParts.includes(file.name));
      }
      updateInputs(media.audio.filter(Boolean));
      const materialize=image=>new File([image.bytes],image.name,{type:image.type});
      if(media.cover)applyCover(materialize(media.cover),refs.cover);
      if(media.backCover)applyBackCover(materialize(media.backCover),refs.backCover);
      if(media.merged)activateRecording(media.merged);
      if(loadedFiles().length)renderInputs();
      const recovered=loadedFiles().length+(media.cover?1:0)+(media.backCover?1:0);
      const missingImages=(refs.cover&&!media.cover?1:0)+(refs.backCover&&!media.backCover?1:0);
      status.textContent=(recovered?'已恢复 '+recovered+' 个本地文件。':'')+
        (missingAudio.length||missingMerged?' 有音频文件需要重新关联。':'')+
        (missingImages?' 有图片文件需要在专辑封面区域重新选择。':'');
    }catch(error){
      status.textContent='恢复本地文件失败。'+error.message;
    }finally{
      if(!disposed){
        restoring=false;
        savedDraft=draftSnapshot();
        renderMissingMedia();
        controls();
      }
    }
  }
  root.projectIdentifier=()=>state.project.id;
  root.dispose=()=>{
    disposed=true;
    state.engine?.close();
    clearProgress?.();
    audio.pause();
    if(state.audioUrl)URL.revokeObjectURL(state.audioUrl);
    if(state.coverUrl)URL.revokeObjectURL(state.coverUrl);
    return clearCache();
  };
  root.hasUnsavedChanges=()=>savedDraft!==draftSnapshot();
  root.isProcessing=()=>state.busy;
  root.stopForExit=async()=>{
    if(!state.busy)return;
    const idle=new Promise(resolve=>{idleWaiter=resolve;});
    state.stop=true;
    (desktop?nativeEngine:state.engine)?.close();
    await idle;
  };
  root.waitForMediaSave=()=>mediaSave||preparingArtwork;
  root.saveForExit=async()=>{
    const {project,rule}=collect(false);
    await storeDraft(project,rule);
    controls();
  };
  renderInputs();
  updateAlbumPath();
  showCover();
  showBackCover();
  savedDraft=draftSnapshot();
  void restoreProjectMedia();
  return root;
}
