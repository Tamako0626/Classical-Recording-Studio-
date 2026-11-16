import {basename,extname,join} from 'node:path';
import {mkdtemp,rm,stat,writeFile} from 'node:fs/promises';
import {coverValid,outputFilenameValid} from '../web/script.js';

const modes=['auto','mp4','mkv48'];
export const videoAudioInput=/\.(wav|flac|mp3|mkv|ape|m4a|aac|aiff?|wv|ogg|opus|mp4)$/iu;
const lossless=codec=>/^(?:flac|alac|ape|tta|pcm_[suf]\d+(?:le|be)?)$/u.test(codec);
function depth(stream){
  return Number(stream.bits_per_raw_sample)||Number(stream.bits_per_sample)||Number(stream.codec_name?.match(/^pcm_[suf](\d+)/u)?.[1])||0;
}
function audioPlan(stream,mode){
  const rate=Number(stream?.sample_rate);
  if(!Number.isInteger(rate)||rate<1)throw Error('无法读取源音频采样率。');
  const isLossless=lossless(stream.codec_name||'');
  const bitDepth=depth(stream);
  if(mode==='mkv48'||mode==='auto'&&rate>=48000&&isLossless){
    const targetRate=Math.max(rate,48000);
    const copy=targetRate===rate&&bitDepth>=24&&['flac','pcm_s24le','pcm_s32le'].includes(stream.codec_name);
    const codec=copy?stream.codec_name:bitDepth>24?'pcm_s32le':'pcm_s24le';
    return {ext:'mkv',codec,rate:targetRate,sourceLossless:isLossless,
      args:copy?['-c:a','copy']:['-c:a',codec,'-ar',String(targetRate)]};
  }
  const targetRate=[32000,44100,48000].includes(rate)?rate:48000;
  return {ext:'mp4',codec:'aac',rate:targetRate,sourceLossless:isLossless,
    args:['-c:a','aac','-b:a','256k','-ar',String(targetRate),'-movflags','+faststart']};
}

export async function makeCoverVideos({sources,cover,folder,mode},{probe,encode,checkCover,publish,signal}){
  if(!modes.includes(mode)||!Array.isArray(sources)||!sources.length||sources.length>500)throw Error('请选择 1 至 500 条音轨和有效的封装格式。');
  if(!coverValid(cover?.name)||!(cover.bytes instanceof Uint8Array)||!cover.bytes.length||cover.bytes.length>30*1024*1024)throw Error('请选择不超过 30 MB 的 PNG 或 JPG 封面。');
  const scratch=await mkdtemp(join(folder,'.studio-video-'));
  const results=[];
  const used=new Set();
  try{
    const image=join(scratch,'cover'+extname(cover.name).toLowerCase()),template=join(scratch,'still.mkv');
    await writeFile(image,cover.bytes);
    await checkCover(image,signal);
    await encode(['-loop','1','-framerate','24','-i',image,'-an','-map','0:v:0',
      '-vf','scale=1920:1080:force_original_aspect_ratio=decrease:force_divisible_by=2,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:black,setsar=1',
      '-c:v','libx264','-preset','ultrafast','-tune','stillimage','-pix_fmt','yuv420p','-threads','2','-bf','0','-g','288','-t','12'],template,signal);
    for(const [i,source] of sources.entries()){
      if(signal.aborted)break;
      let target='';
      try{
        const info=await probe(source,signal);
        const audio=info.streams?.find(stream=>stream.codec_type==='audio');
        const audioLength=Number(audio?.duration)||Number(info.format?.duration);
        if(!Number.isFinite(audioLength)||audioLength<=0)throw Error('无法读取源音频长度。');
        const plan=audioPlan(audio,mode);
        const stem=basename(source,extname(source)).slice(0,190).replace(/[. ]+$/u,'');
        target=stem+'.'+plan.ext;
        for(let count=2;used.has(target.toLowerCase());count++)target=stem+' ('+count+').'+plan.ext;
        if(!outputFilenameValid(target))throw Error('输出文件名无效。');
        used.add(target.toLowerCase());
        const destination=join(folder,target);
        const exists=await stat(destination).then(()=>true,error=>{if(error.code==='ENOENT')return false;throw error;});
        if(exists)throw Error('输出目录已有同名文件。');
        const staged=join(scratch,'video-'+i+'.'+plan.ext);
        await encode(['-stream_loop','-1','-i',template,'-i',source,'-map','0:v:0','-map','1:a:0','-map_metadata','1','-map_chapters','-1',
          '-c:v','copy',...plan.args,'-shortest'],staged,signal);
        const output=await probe(staged,signal);
        const video=output.streams?.find(stream=>stream.codec_type==='video');
        const recorded=output.streams?.find(stream=>stream.codec_type==='audio');
        const duration=Number(recorded?.duration)||Number(output.format?.duration);
        const outputDepth=depth(recorded||{});
        if(video?.codec_name!=='h264'||video.width!==1920||video.height!==1080||recorded?.codec_name!==plan.codec||
          Number(recorded?.sample_rate)!==plan.rate||plan.ext==='mkv'&&outputDepth<24||!Number.isFinite(duration)||Math.abs(duration-audioLength)>.25){
          throw Error('成品的画面、音频格式或时长未通过检查。');
        }
        signal.throwIfAborted();
        try{await publish(staged,destination);}
        catch(error){if(error.code==='EEXIST')throw Error('输出目录已有同名文件。');throw error;}
        results.push({source:basename(source),target,path:destination,rate:plan.rate,bitDepth:outputDepth,codec:plan.codec,
          sourceRate:Number(audio.sample_rate),sourceBitDepth:depth(audio),sourceLossless:plan.sourceLossless});
      }catch(error){
        if(signal.aborted)break;
        results.push({source:basename(source),target,error:error.message});
      }
    }
    return {results,stopped:signal.aborted};
  }finally{await rm(scratch,{recursive:true,force:true});}
}
