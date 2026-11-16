import {makeCoverVideos} from './cover-video.mjs';
import {effectPlan,fadeFilters} from '../web/audio-effects.js';
import {spawn} from 'node:child_process';
import {basename,extname,join} from 'node:path';
import {copyFile,link,lstat,mkdir,mkdtemp,readFile,realpath,rm,stat,writeFile} from 'node:fs/promises';
import {constants} from 'node:fs';
import {tmpdir} from 'node:os';
import {tags} from '../web/seed.js';
import {coverValid,outputFilenameValid,outputValid} from '../web/script.js';

const encoders={
  FLAC:['-c:a','flac','-sample_fmt','s32','-bits_per_raw_sample','24'],
  WAV:['-c:a','pcm_s24le','-rf64','auto'],
  MP3:['-c:a','libmp3lame','-q:a','2','-id3v2_version','3'],
  MKV:['-c:a','pcm_s24le']
};
async function publishNewFile(source,destination){
  try{await link(source,destination);}
  catch(error){
    if(!['EPERM','ENOTSUP','EOPNOTSUPP','ENOSYS','EXDEV'].includes(error.code))throw error;
    await copyFile(source,destination,constants.COPYFILE_EXCL);
  }
}
export async function albumOutputDirectory(parent,name){
  if(typeof name!=='string'||!outputValid(name))throw Error('专辑文件夹名称无效。');
  const base=await realpath(parent);
  const path=join(base,name);
  try{await mkdir(path);}
  catch(error){if(error.code!=='EEXIST')throw error;}
  const info=await lstat(path);
  if(!info.isDirectory()||info.isSymbolicLink())throw Error('专辑输出位置存在同名文件或目录链接。');
  return path;
}

export function createNativeProcessor({ffmpeg='ffmpeg',ffprobe='ffprobe',onProgress=()=>{}}={}){
  let controller;
  const cancel=()=>controller?.abort();
  async function execute(binary,args,signal,report=false){
    signal?.throwIfAborted();
    return new Promise((resolve,reject)=>{
      const child=spawn(binary,args,{windowsHide:true,stdio:['ignore','pipe','pipe'],signal});
      let output='', errors='', progress='';
      child.stdout.setEncoding('utf8');
      child.stderr.setEncoding('utf8');
      child.stdout.on('data',chunk=>{
        if(!report){output=(output+chunk).slice(-2_000_000);return;}
        progress+=chunk;
        const lines=progress.split('\n');progress=lines.pop();
        for(const line of lines){
          const value=line.match(/^out_time_us=(\d+)/)?.[1];
          if(value)onProgress(Number(value)/1e6);
        }
      });
      child.stderr.on('data',chunk=>{errors=(errors+chunk).slice(-7000);});
      child.once('error',error=>{
        if(error.code==='ENOENT'){
          const install=process.platform==='darwin'?'请在终端运行 brew install ffmpeg，再重新启动软件。':'请安装 FFmpeg，将 ffmpeg 和 ffprobe 加入 PATH，再重新启动软件。';
          reject(Error('找不到 '+binary+'。'+install));
        }
        else reject(error);
      });
      child.once('close',code=>{
        if(signal?.aborted)reject(Error('已停止处理。'));
        else if(code!==0)reject(Error(errors.trim()||binary+' 执行失败（'+code+'）。'));
        else resolve(output);
      });
    });
  }
  async function probe(path,signal){
    return JSON.parse(await execute(ffprobe,['-v','error','-show_format','-show_streams','-of','json',path],signal));
  }
  function audioInfo(info){
    const stream=info.streams?.find(item=>item.codec_type==='audio');
    if(!stream)throw Error('文件没有可用音轨。');
    return {duration:Number(stream.duration)||Number(info.format?.duration)||null,channels:stream.channels,rate:Number(stream.sample_rate),layout:stream.channel_layout};
  }
  const inspect=async(path,signal)=>audioInfo(await probe(path,signal));
  const encode=(args,path,signal)=>execute(ffmpeg,['-hide_banner','-nostdin','-v','error','-xerror','-progress','pipe:1','-nostats',...args,path],signal,true);
  async function coverInfo(path,signal){
    const mime=extname(path).toLowerCase()==='.png'?'image/png':'image/jpeg';
    const codec=(await probe(path,signal)).streams?.find(item=>item.codec_type==='video')?.codec_name;
    if(codec!==(mime==='image/png'?'png':'mjpeg'))throw Error('封面扩展名与图像格式不一致。');
    return mime;
  }
  async function archiveCover({folder,cover,back=false},signal){
    const label=back?'封底':'封面';
    if(!coverValid(cover?.name)||!(cover.bytes instanceof Uint8Array)||!cover.bytes.length||cover.bytes.length>30*1024*1024)throw Error(label+'文件无效。');
    const name=(back?'back':'cover')+(extname(cover.name).toLowerCase()==='.png'?'.png':'.jpg');
    const scratch=await mkdtemp(join(folder,'.studio-cover-'));
    const staged=join(scratch,name),destination=join(folder,name);
    try{
      await writeFile(staged,cover.bytes);
      try{await coverInfo(staged,signal);}
      catch(error){if(back&&!signal.aborted)throw Error('封底：'+error.message);throw error;}
      signal.throwIfAborted();
      try{await publishNewFile(staged,destination);}
      catch(error){
        if(error.code!=='EEXIST')throw error;
        const info=await lstat(destination);
        if(!info.isFile()||info.size!==cover.bytes.length||!(await readFile(destination)).equals(Buffer.from(cover.bytes))){
          throw Error('专辑文件夹已有不同的 '+name+'。请保留现有图片并选择另一个专辑文件夹，或移走原文件后重试。');
        }
      }
      return destination;
    }finally{await rm(scratch,{recursive:true,force:true});}
  }
  function validateAudioSettings(job,settings){
    if(!Number.isFinite(job.start)||job.start<0||job.end!==null&&(!Number.isFinite(job.end)||job.end<=job.start))throw Error('分轨时间无效。');
    if(![44100,48000,96000].includes(settings.rate))throw Error('采样率无效。');
    for(const [name,min,max] of [['gain',-60,30],['width',0,250],['reverb',0,100]]){
      if(!Number.isFinite(settings[name])||settings[name]<min||settings[name]>max)throw Error('音频处理设置无效。');
    }
  }
  async function trackFilters(source,job,settings,signal,media){
    const effects=effectPlan(settings.effects);
    const needsProbe=effects.stereo||settings.width!==100||effects.fadeOut&&job.end===null;
    if(needsProbe&&!media)media=await inspect(source,signal);
    if(effects.stereo&&media.channels!==2)throw Error('交换声道或反转右声道极性需要双声道录音。');
    if(settings.width!==100&&media.channels!==2)throw Error('声场宽度调整需要双声道录音。');
    const duration=job.end===null?(media?.duration?media.duration-job.start:null):job.end-job.start;
    const filters=[...effects.before];
    if(settings.gain!==0)filters.push('volume='+settings.gain+'dB');
    if(settings.width!==100)filters.push('extrastereo=c=0:m='+settings.width/100);
    if(settings.reverb>0){
      const mix=settings.reverb/100;
      filters.push('aecho=1:1:35|61|89:'+ [0.22,0.14,0.08].map(x=>(mix*x).toFixed(5)).join('|'));
    }
    return [...filters,...effects.after,...fadeFilters(effects,duration),...effects.tail];
  }
  async function track({source,cover,folder,job,format,settings},signal){
    if(!Object.hasOwn(encoders,format)||!outputFilenameValid(job.target)||extname(job.target).toLowerCase()!=='.'+format.toLowerCase())throw Error('输出文件格式或文件名无效。');
    validateAudioSettings(job,settings);
    if(cover&&format==='WAV')throw Error('需要嵌入封面时请选择 FLAC。');
    const coverMime=cover?await coverInfo(cover,signal):null;
    const args=['-ss',String(job.start),'-i',source];
    if(cover&&format!=='MKV')args.push('-i',cover);
    args.push('-map','0:a:0','-map_chapters','-1','-map_metadata','-1');
    if(job.end!==null)args.push('-t',String(job.end-job.start));
    for(const key of tags){
      const value=job.tags?.[key];
      if(typeof value==='string'&&value&&!value.includes('\0'))args.push('-metadata',key+'='+value.slice(0,1000));
    }
    const filters=await trackFilters(source,job,settings,signal);
    if(filters.length)args.push('-af',filters.join(','));
    args.push('-ar',String(settings.rate),...encoders[format]);
    if(cover&&format!=='MKV')args.push('-map','1:v:0','-c:v','copy','-disposition:v:0','attached_pic','-metadata:s:v:0','title=Album cover','-metadata:s:v:0','comment=Cover (front)');
    if(cover&&format==='MKV')args.push('-attach',cover,'-metadata:s:t:0','mimetype='+coverMime,'-metadata:s:t:0','filename='+basename(cover));
    const destination=join(folder,job.target);
    try{await stat(destination);throw Error('输出目录已有同名文件：'+job.target);}
    catch(error){if(error.code!=='ENOENT')throw error;}
    const scratch=await mkdtemp(join(folder,'.studio-'));
    const output=join(scratch,'track'+extname(job.target));
    try{
      await encode(args,output,signal);
      const info=await probe(output,signal);
      const {duration,rate}=audioInfo(info);
      if(!duration||rate!==settings.rate||job.end!==null&&Math.abs(duration-(job.end-job.start))>0.3)throw Error('质量检查失败：时长或采样率异常。');
      if(cover&&!info.streams.some(stream=>stream.disposition?.attached_pic||stream.codec_type==='attachment'))throw Error('质量检查失败：封面缺失。');
      const metadata=Object.fromEntries(Object.entries(info.format?.tags||{}).map(([key,value])=>[key.toLowerCase(),value]));
      const checkedTags=format==='WAV'?['Title','Artist','Album','Track','Date']:tags;
      for(const key of checkedTags){
        const expected=job.tags?.[key],actual=metadata[key.toLowerCase()];
        if(!expected||expected===actual)continue;
        if(key==='Track'&&/^\d+$/.test(expected)&&/^\d+$/.test(actual)&&Number(expected)===Number(actual))continue;
        throw Error('质量检查失败：'+key+' 标签未正确写入。');
      }
      await execute(ffmpeg,['-nostdin','-v','error','-xerror','-i',output,'-map','0:a:0','-f','null','-'],signal);
      signal.throwIfAborted();
      // Publish only a verified output. link fails atomically if a same-named file appeared.
      try{await publishNewFile(output,destination);}
      catch(error){if(error.code==='EEXIST')throw Error('输出目录已有同名文件：'+job.target);throw error;}
      return {duration,path:destination};
    }finally{await rm(scratch,{recursive:true,force:true});}
  }
  async function concatenate(paths,output,signal){
    const info=[];
    for(const path of paths)info.push(await inspect(path,signal));
    const rate=Math.max(...info.map(item=>item.rate));
    const channels=Math.max(...info.map(item=>item.channels));
    const layout=channels===1?'mono':channels===2?'stereo':info[0].layout;
    if(channels>2&&(!layout||info.some(item=>item.layout!==layout||item.channels!==channels)))throw Error('多声道拼接要求所有片段使用相同声道布局。');
    const filters=paths.map((_,i)=>'['+i+':a:0]aformat=sample_fmts=s32:sample_rates='+rate+':channel_layouts='+layout+',asetpts=N/SR/TB[a'+i+']');
    filters.push(paths.map((_,i)=>'[a'+i+']').join('')+'concat=n='+paths.length+':v=0:a=1[joined]');
    await encode([...paths.flatMap(path=>['-i',path]),'-filter_complex',filters.join(';'),'-map','[joined]','-map_metadata','-1',...encoders.FLAC],output,signal);
    return inspect(output,signal);
  }
  async function preview(source,start,signal){
    if(!Number.isFinite(start)||start<0)throw Error('试听起点无效。');
    const dir=await mkdtemp(join(tmpdir(),'studio-preview-'));
    const path=join(dir,'preview.mp3');
    try{
      await encode(['-ss',String(start),'-i',source,'-map','0:a:0','-t','20','-c:a','libmp3lame','-q:a','5'],path,signal);
      const {readFile}=await import('node:fs/promises');
      return {bytes:await readFile(path)};
    }finally{await rm(dir,{recursive:true,force:true});}
  }
  async function previewEffects({source,job,settings,segment},signal){
    validateAudioSettings(job,settings);
    if(!['beginning','ending'].includes(segment))throw Error('请选择有效的试听片段。');
    const media=await inspect(source,signal);
    if(media.duration&&(job.start>=media.duration||job.end!==null&&job.end>media.duration+0.05))throw Error('试听的起止点超出录音时长。');
    const end=job.end??media.duration;
    const length=end===null?null:end-job.start;
    if(segment==='ending'&&length===null)throw Error('试听结尾需要已知时长，请先填写终点。');
    const span=Math.min(20,length??20);
    const offset=segment==='ending'?Math.max(0,length-span):0;
    const filters=await trackFilters(source,job,settings,signal,media);
    const dir=await mkdtemp(join(tmpdir(),'studio-effects-preview-'));
    const original=join(dir,'original.wav'),processed=join(dir,'processed.wav');
    const common=['-map','0:a:0','-map_metadata','-1','-t',String(span),'-ar',String(settings.rate),'-c:a','pcm_s16le'];
    try{
      await encode(['-ss',String(job.start+offset),'-i',source,...common],original,signal);
      if(!filters.length){
        const bytes=await readFile(original);
        return {original:bytes,processed:bytes,start:job.start+offset,duration:span};
      }
      // Output seeking runs filters on the material preceding the requested excerpt.
      await encode(['-ss',String(job.start),'-i',source,'-ss',String(offset),'-af',filters.join(','),...common],processed,signal);
      const [raw,wet]=await Promise.all([readFile(original),readFile(processed)]);
      return {original:raw,processed:wet,start:job.start+offset,duration:span};
    }finally{await rm(dir,{recursive:true,force:true});}
  }
  async function run(action,payload){
    if(controller)throw Error('FFmpeg 正在处理其他操作。');
    controller=new AbortController();
    const {signal}=controller;
    try{
      if(action==='prepare'){
        if(payload.cover)await coverInfo(payload.cover,signal);
        return await inspect(payload.source,signal);
      }
      if(action==='metadata'){
        const info=await probe(payload.source,signal);
        const stream=info.streams.find(item=>item.codec_type==='audio');
        const fields=new Set(['title','artist','album','track','date','composer','conductor','soloist','orchestra','disc','album_artist','genre']);
        const tags=Object.fromEntries(Object.entries({...stream?.tags,...info.format?.tags}).filter(([key,value])=>fields.has(key.toLowerCase())&&typeof value==='string').map(([key,value])=>[key,value.slice(0,350)]));
        return {...audioInfo(info),codec:stream?.codec_name,tags};
      }
      if(action==='track')return await track(payload,signal);
      if(action==='cover-videos')return await makeCoverVideos(payload,{probe,encode,checkCover:coverInfo,publish:publishNewFile,signal});
      if(action==='archive-album-artwork'){
        const {folder,cover,backCover}=payload;
        const backPath=backCover?await archiveCover({folder,cover:backCover,back:true},signal):null;
        const coverPath=cover?await archiveCover({folder,cover},signal):null;
        return {coverPath,backPath};
      }
      if(action==='archive-cover')return await archiveCover(payload,signal);
      if(action==='preview-effects')return await previewEffects(payload,signal);
      if(action==='preview')return await preview(payload.source,payload.start,signal);
      if(action==='concatenate')return await concatenate(payload.sources,payload.output,signal);
      throw Error('不支持此音频操作。');
    }finally{controller=null;}
  }
  return {run,cancel};
}
