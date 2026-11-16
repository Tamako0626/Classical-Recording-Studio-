/* global createFFmpegCore */
let core;
let mounted = false;
let logs = [];
let activeTrack = false;
const version = '0.12.10';
const integrity = 'sha384-U1VDhkPYrM3wTCT4/vjSpSsKqG/UjljYrYCI4hBSJ02svbCkxuCi6U6u/peg5vpW';
const mirrors = [
  'https://cdn.jsdelivr.net/npm/@ffmpeg/core@'+version+'/dist/umd/ffmpeg-core.wasm',
  'https://unpkg.com/@ffmpeg/core@'+version+'/dist/umd/ffmpeg-core.wasm'
];
async function loadCore() {
  importScripts('/ffmpeg-core.js');
  let binary;
  for (const url of mirrors) {
    try {
      const response = await fetch(url,{integrity,credentials:'omit',referrerPolicy:'no-referrer',signal:AbortSignal.timeout(45000)});
      if (response.ok) {binary=await response.arrayBuffer();break;}
    } catch {}
  }
  if (!binary) throw Error('无法加载 FFmpeg 引擎，请检查网络后重试。');
  core = await createFFmpegCore({wasmBinary:new Uint8Array(binary)});
  core.setLogger(({message})=>{
    logs.push(message);
    if (logs.length>80) logs.shift();
  });
  core.setProgress(({time})=>{
    if (activeTrack) self.postMessage({kind:'progress',seconds:time/1000000});
  });
  core.FS.mkdir('/source');
}
function execute(mode,args) {
  logs=[];
  core[mode](...args);
  const code=core.ret;
  core.reset();
  if(code!==0&&!(mode==='ffprobe'&&code===-1)) throw Error(logs.join('\n').slice(-3500)||'FFmpeg 执行失败（'+code+'）');
}
function remove(path){try{core.FS.unlink(path);}catch{}}
function probe(path) {
  const report='/info.json';
  try {
    execute('ffprobe',['-v','error','-show_format','-show_streams','-of','json','-o',report,path]);
    try{return JSON.parse(core.FS.readFile(report,{encoding:'utf8'}));}
    catch{throw Error(logs.join('\n').slice(-3500)||'FFprobe 无法读取此文件。');}
  } finally {remove(report);}
}
function media(info) {
  const audio=info.streams?.find(stream=>stream.codec_type==='audio');
  if(!audio)throw Error('文件中没有可用的音轨。');
  const duration=Number(audio.duration)||Number(info.format?.duration);
  return {duration:Number.isFinite(duration)&&duration>0?duration:null,channels:Number(audio.channels),rate:Number(audio.sample_rate)};
}
let source, image;
async function concatenate(files) {
  if(!core)await loadCore();
  if(mounted){core.FS.unmount('/source');mounted=false;}
  const blobs=files.map((file,index)=>({name:'input'+index+file.name.slice(file.name.lastIndexOf('.')).toLowerCase(),data:file}));
  const output='/joined.flac';
  try{
    core.FS.mount(core.FS.filesystems.WORKERFS,{blobs},'/source');
    mounted=true;
    const paths=blobs.map(blob=>'/source/'+blob.name);
    const inputs=paths.map((path,index)=>{
      try{
        const info=probe(path);
        const audio=info.streams?.find(stream=>stream.codec_type==='audio');
        return {...media(info),layout:audio.channel_layout};
      }catch(error){throw Error('第 '+(index+1)+' 段（'+files[index].name+'）：'+error.message);}
    });
    const rate=Math.max(...inputs.map(input=>input.rate));
    const channels=Math.max(...inputs.map(input=>input.channels));
    const layout=channels===1?'mono':channels===2?'stereo':inputs[0].layout;
    if(channels>2&&(!layout||inputs.some(input=>input.channels!==channels||input.layout!==layout))){
      throw Error('多声道录音需要使用相同声道布局后再拼接。');
    }
    const filters=paths.map((_,i)=>'['+i+':a:0]aformat=sample_fmts=s32:sample_rates='+rate+':channel_layouts='+layout+
      ',asetpts=N/SR/TB[a'+i+']');
    filters.push(paths.map((_,i)=>'[a'+i+']').join('')+'concat=n='+paths.length+':v=0:a=1[joined]');
    const maxBytes=450*1024*1024;
    const duration=inputs.every(input=>input.duration!==null)?inputs.reduce((sum,input)=>sum+input.duration,0):null;
    self.postMessage({kind:'merge-duration',seconds:duration});
    activeTrack=true;
    execute('exec',['-v','error','-xerror',...paths.flatMap(path=>['-i',path]),'-filter_complex',filters.join(';'),
      '-map','[joined]','-map_chapters','-1','-map_metadata','-1',...encoders.FLAC,'-fs',String(maxBytes),output]);
    activeTrack=false;
    if(core.FS.stat(output).size>=maxBytes-1024*1024)throw Error('拼接输出接近浏览器单文件上限（450 MB）。请分批拼接或在本机合并。');
    const joined=media(probe(output));
    if(!joined.duration||joined.rate!==rate||joined.channels!==channels)throw Error('拼接结果质量检查未通过。');
    return {...joined,bytes:core.FS.readFile(output)};
  }finally{
    activeTrack=false;remove(output);
    if(mounted){core.FS.unmount('/source');mounted=false;}
    source=image=null;
  }
}
async function prepare({file,cover}) {
  if(!core)await loadCore();
  if(mounted){core.FS.unmount('/source');mounted=false;}
  const blobs=[{name:'recording'+file.name.slice(file.name.lastIndexOf('.')).toLowerCase(),data:file}];
  if(cover)blobs.push({name:cover.name.toLowerCase().endsWith('.png')?'cover.png':'cover.jpg',data:cover});
  core.FS.mount(core.FS.filesystems.WORKERFS,{blobs},'/source');
  mounted=true;
  source='/source/'+blobs[0].name;
  image=null;
  if(cover){
    const path='/source/'+blobs[1].name;
    const codec=probe(path).streams?.find(s=>s.codec_type==='video')?.codec_name;
    const isPNG=path.endsWith('.png');
    if(codec!==(isPNG?'png':'mjpeg'))throw Error('封面的文件扩展名与图像格式不一致。');
    image={path,mime:isPNG?'image/png':'image/jpeg'};
  }
  return media(probe(source));
}
const encoders={
  FLAC:['-c:a','flac','-sample_fmt','s32','-bits_per_raw_sample','24'],
  WAV:['-c:a','pcm_s24le'],
  MP3:['-c:a','libmp3lame','-q:a','2','-id3v2_version','3'],
  MKV:['-c:a','pcm_s24le']
};
async function processTrack({job,settings,format}) {
  const output='/output.'+format.toLowerCase();
  const args=['-v','error','-xerror','-ss',String(job.start),'-i',source];
  if(image&&format!=='MKV')args.push('-i',image.path);
  args.push('-map','0:a:0','-map_chapters','-1','-map_metadata','-1');
  if(job.end!==null)args.push('-t',String(job.end-job.start));
  for(const [key,value] of Object.entries(job.tags))args.push('-metadata',key+'='+value);
  const {effectPlan,fadeFilters}=await import('/audio-effects.js');
  const effects=effectPlan(settings.effects);
  const info=effects.stereo||effects.fadeOut&&job.end===null?media(probe(source)):null;
  if(effects.stereo&&info.channels!==2)throw Error('交换声道或反转右声道极性需要双声道录音。');
  const length=job.end===null?(info?.duration?info.duration-job.start:null):job.end-job.start;
  const filters=[...effects.before];
  if(settings.gain!==0)filters.push('volume='+settings.gain+'dB');
  if(settings.width!==100)filters.push('extrastereo=c=0:m='+settings.width/100);
  if(settings.reverb>0){
    const mix=settings.reverb/100;
    filters.push('aecho=1:1:35|61|89:'+ [0.22,0.14,0.08].map(x=>(mix*x).toFixed(5)).join('|'));
  }
  filters.push(...effects.after,...fadeFilters(effects,length),...effects.tail);
  if(filters.length)args.push('-af',filters.join(','));
  args.push('-ar',String(settings.rate),...encoders[format]);
  if(image&&format!=='MKV')args.push('-map','1:v:0','-c:v','copy','-disposition:v:0','attached_pic','-metadata:s:v:0','title=Album cover','-metadata:s:v:0','comment=Cover (front)');
  if(image&&format==='MKV')args.push('-attach',image.path,'-metadata:s:t:0','mimetype='+image.mime,'-metadata:s:t:0','filename='+image.path.split('/').pop());
  try {
    activeTrack=true;
    execute('exec',[...args,output]);
    activeTrack=false;
    const info=probe(output);
    const {rate,duration}=media(info);
    if(!duration||rate!==settings.rate)throw Error('质量检查失败：时长或采样率异常。');
    if(job.end!==null&&Math.abs(duration-(job.end-job.start))>0.3)throw Error('质量检查失败：分轨时长与预期不符。');
    if(image&&!info.streams.some(s=>s.disposition?.attached_pic||s.codec_type==='attachment'))throw Error('质量检查失败：未找到封面。');
    const metadata=Object.fromEntries(Object.entries(info.format?.tags||{}).map(([key,value])=>[key.toLowerCase(),value]));
    const supportedTags=format==='WAV'?['Title','Artist','Album','Track','Date']:Object.keys(job.tags);
    for(const key of supportedTags){
      const expected=job.tags[key];
      const actual=metadata[key.toLowerCase()];
      if(expected&&(key==='Track'&&/^\d+$/.test(expected)&&/^\d+$/.test(actual)?Number(actual)!==Number(expected):actual!==expected))throw Error('质量检查失败：'+key+' 标签未正确写入。');
    }
    execute('exec',['-v','error','-xerror','-i',output,'-map','0:a:0','-f','null','-']);
    return {duration,bytes:core.FS.readFile(output)};
  } finally {activeTrack=false;remove(output);}
}
function preview(start) {
  const output='/preview.mp3';
  try {
    execute('exec',['-v','error','-ss',String(start),'-i',source,'-map','0:a:0','-t','20','-c:a','libmp3lame','-q:a','5',output]);
    return {bytes:core.FS.readFile(output)};
  } finally {remove(output);}
}
self.onmessage=async ({data:{id,action,payload}})=>{
  try {
    let value;
    if(action==='prepare')value=await prepare(payload);
    if(action==='concatenate')value=await concatenate(payload);
    if(action==='track')value=await processTrack(payload);
    if(action==='preview')value=preview(payload);
    self.postMessage({id,value},value?.bytes?[value.bytes.buffer]:[]);
  }catch(error){self.postMessage({id,error:error.message});}
};
