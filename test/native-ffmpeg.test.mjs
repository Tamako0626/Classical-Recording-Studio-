import {test} from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdtemp,readFile,rm,stat,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {sessionFolderName} from '../web/session-english.js';
import {albumOutputDirectory,createNativeProcessor} from '../desktop/native-ffmpeg.mjs';

test('native FFmpeg concatenates, previews, cuts and embeds cover without overwriting',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'studio-native-test-'));
  const ff=(args)=>execFileSync('ffmpeg',['-hide_banner','-v','error',...args]);
  const source=join(dir,'第一段.wav'),other=join(dir,'second.mp3'),cover=join(dir,'cover.png');
  const joined=join(dir,'joined.flac');
  const processor=createNativeProcessor();
  try{
    ff(['-f','lavfi','-i','sine=frequency=330:sample_rate=44100:duration=1',source]);
    ff(['-f','lavfi','-i','sine=frequency=660:sample_rate=48000:duration=1.5','-ac','2',other]);
    ff(['-f','lavfi','-i','color=blue:size=64x64','-frames:v','1','-threads','1',cover]);
    const media=await processor.run('concatenate',{sources:[source,other],output:joined});
    assert.equal(media.rate,48000);
    assert.ok(Math.abs(media.duration-2.5)<0.05);
    assert.ok((await processor.run('preview',{source:joined,start:1})).bytes.length);
    const session={date:'2026-11-06',performer:'Michel Dalberto',city:'上海',name:'Michel Dalberto · Mozart Recital',origin:'现场自录'};
    const name='2026-11-06 Michel Dalberto - Shanghai Mozart Recital (Live Recording)';
    assert.equal(sessionFolderName(session),name);
    assert.equal(sessionFolderName({...session,name}),name);
    const folder=await albumOutputDirectory(dir,name);
    assert.equal(await albumOutputDirectory(dir,name),folder);
    await assert.rejects(albumOutputDirectory(dir,'../invalid'),/无效/);
    await assert.rejects(albumOutputDirectory(dir,'cover.png'),/同名/);
    const originalBytes=await readFile(cover);
    const inputCover={name:'扫描封面.PNG',bytes:originalBytes};
    const coverPath=await processor.run('archive-cover',{folder,cover:inputCover});
    assert.equal(coverPath,join(folder,'cover.png'));
    assert.deepEqual(await readFile(coverPath),originalBytes);
    assert.equal(await processor.run('archive-cover',{folder,cover:inputCover}),coverPath);
    const jpeg=join(dir,'other.jpeg');
    ff(['-f','lavfi','-i','color=red:size=64x64','-frames:v','1','-threads','1',jpeg]);
    const jpegBytes=await readFile(jpeg);
    assert.equal(await processor.run('archive-cover',{folder,cover:{name:'Scan.JPEG',bytes:jpegBytes}}),join(folder,'cover.jpg'));
    assert.deepEqual(await readFile(join(folder,'cover.jpg')),jpegBytes);
    await assert.rejects(processor.run('archive-cover',{folder,cover:{name:'Wrong.PNG',bytes:jpegBytes}}),/扩展名/);
    const different=join(dir,'different.png');
    ff(['-f','lavfi','-i','color=red:size=64x64','-frames:v','1','-threads','1',different]);
    await assert.rejects(processor.run('archive-cover',{folder,cover:{name:'different.png',bytes:await readFile(different)}}),/已有不同/);
    const artwork=await processor.run('archive-album-artwork',{folder,cover:inputCover,backCover:{name:'reverse.jpeg',bytes:jpegBytes}});
    assert.deepEqual(artwork,{coverPath,backPath:join(folder,'back.jpg')});
    assert.deepEqual(await readFile(artwork.backPath),jpegBytes);
    const withNoBack=await processor.run('archive-album-artwork',{folder,cover:inputCover});
    assert.deepEqual(withNoBack,{coverPath,backPath:null});
    await assert.rejects(processor.run('archive-album-artwork',{folder,backCover:{name:'wrong.png',bytes:jpegBytes}}),/封底/);
    const backOnlyFolder=await albumOutputDirectory(dir,'Back only');
    const backOnly=await processor.run('archive-album-artwork',{folder:backOnlyFolder,backCover:{name:'reverse.png',bytes:originalBytes}});
    assert.deepEqual(backOnly,{coverPath:null,backPath:join(backOnlyFolder,'back.png')});
    await assert.rejects(stat(join(backOnlyFolder,'cover.png')), {code:'ENOENT'});
    const task={source:joined,cover:coverPath,folder,job:{target:'01 - Allegro.flac',start:0.5,end:2,tags:{Title:'Allegro',Composer:'Robert Schumann'}},format:'FLAC',settings:{rate:48000,gain:2,width:120,reverb:0}};
    const result=await processor.run('track',task);
    assert.equal(result.duration,1.5);
    assert.equal(result.path,join(folder,task.job.target));
    const meta=JSON.parse(execFileSync('ffprobe',['-v','error','-show_format','-show_streams','-of','json',result.path],{encoding:'utf8'}));
    assert.ok(meta.streams.some(stream=>stream.disposition?.attached_pic));
    const tags=Object.fromEntries(Object.entries(meta.format.tags).map(([key,value])=>[key.toLowerCase(),value]));
    assert.equal(tags.title,'Allegro');
    await assert.rejects(processor.run('track',task),/同名/);
    assert.ok((await stat(source)).size>0);
  }finally{await rm(dir,{recursive:true,force:true});}
});

test('advanced effects run with native FFmpeg and preserve source and track duration',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'studio-effects-test-'));
  const source=join(dir,'source.wav');
  const processor=createNativeProcessor();
  try{
    execFileSync('ffmpeg',['-v','error','-f','lavfi','-i','sine=frequency=330:sample_rate=44100:duration=4','-ac','2','-metadata','title=Original',source]);
    const input=await readFile(source);
    const job={target:'denoised.flac',start:0.25,end:null,tags:{Title:'Processed'}};
    const effects={denoise:'fft',noiseReduction:8,hum:'50',declip:true,declick:true,highpass:30,lowpass:16000,bass:1,mid:-1,treble:1,gate:-60,
      channels:'swap',compress:2,threshold:-20,deess:true,normalize:'loud',lufs:-18,limiter:-1,fadeIn:.2,fadeOut:.3};
    const settings={rate:48000,gain:0,width:100,reverb:0,effects};
    const result=await processor.run('track',{source,folder:dir,job,format:'FLAC',settings});
    const metadata=await processor.run('metadata',{source:result.path});
    assert.equal(metadata.rate,48000);
    assert.equal(metadata.channels,2);
    assert.ok(Math.abs(result.duration-3.75)<0.1);
    assert.equal(metadata.tags.Title||metadata.tags.title,'Processed');
    assert.deepEqual(await readFile(source),input);
    const second=await processor.run('track',{source,folder:dir,job:{...job,target:'mono.flac',end:2},format:'FLAC',
      settings:{...settings,effects:{denoise:'nlm',channels:'mono',normalize:'dynamic'}}});
    assert.equal((await processor.run('metadata',{source:second.path})).channels,1);
  }finally{await rm(dir,{recursive:true,force:true});}
});

test('effect preview aligns ending with the source and renders the same fade as the export',async()=>{
  const folder=await mkdtemp(join(tmpdir(),'studio-preview-test-'));
  const source=join(folder,'source.wav'),processor=createNativeProcessor();
  const decode=path=>execFileSync('ffmpeg',['-v','error','-i',path,'-c:a','pcm_s16le','-f','s16le','-'],{maxBuffer:8*1024*1024});
  function average(buffer,from,to){
    let total=0;
    for(let i=from;i<to;i++)total+=Math.abs(buffer.readInt16LE(i*2));
    return total/(to-from);
  }
  try{
    execFileSync('ffmpeg',['-v','error','-f','lavfi','-i','sine=frequency=330:sample_rate=48000:duration=28',source]);
    const settings={rate:48000,gain:6,width:100,reverb:0,effects:{fadeOut:4}};
    const job={target:'track.flac',start:2,end:26,tags:{}};
    const {original,processed,start,duration}=await processor.run('preview-effects',{source,job,settings,segment:'ending'});
    assert.equal(start,6);
    assert.equal(duration,20);
    const rawPath=join(folder,'original.wav'),wetPath=join(folder,'processed.wav');
    await writeFile(rawPath,original);
    await writeFile(wetPath,processed);
    const raw=decode(rawPath),wet=decode(wetPath);
    assert.equal(raw.length,20*48000*2);
    assert.equal(wet.length,raw.length);
    assert.ok(average(wet,0,48000)>average(raw,0,48000)*1.9);
    assert.ok(average(wet,950000,960000)<average(raw,950000,960000)*.1);
    const exportResult=await processor.run('track',{source,folder,job,settings,format:'FLAC'});
    const exported=decode(exportResult.path).subarray(4*48000*2);
    for(const [from,to] of [[0,48000],[950000,960000]])assert.ok(Math.abs(average(wet,from,to)-average(exported,from,to))<1);
    await assert.rejects(processor.run('preview-effects',{source,job:{...job,start:30,end:null},settings,segment:'ending'}),/超出/);
  }finally{await rm(folder,{recursive:true,force:true});}
});

test('optional gain limiter prevents clipping at +24 dB in export and preview',async()=>{
  const folder=await mkdtemp(join(tmpdir(),'studio-limiter-test-'));
  const source=join(folder,'source.wav'),processor=createNativeProcessor();
  function peak(bytes){
    const argv=['-v','error','-i','pipe:0','-c:a','pcm_s16le','-f','s16le','-'];
    const data=execFileSync('ffmpeg',argv,{input:bytes});
    let maximum=0;
    for(let i=0;i<data.length;i+=2)maximum=Math.max(maximum,Math.abs(data.readInt16LE(i)));
    return 20*Math.log10(maximum/32768);
  }
  try{
    execFileSync('ffmpeg',['-v','error','-f','lavfi','-i','sine=frequency=330:sample_rate=48000:duration=1',source]);
    const original=await readFile(source);
    const settings={rate:48000,gain:24,width:100,reverb:0};
    const job={target:'limited.flac',start:0,end:1,tags:{}};
    const action={source,job,settings,segment:'beginning'};
    const clipped=await processor.run('preview-effects',action);
    assert.ok(peak(clipped.processed)>-.01);
    action.settings={...settings,effects:{limiter:-1}};
    const limited=await processor.run('preview-effects',action);
    assert.ok(Math.abs(peak(limited.processed)+1)<.02);
    const result=await processor.run('track',{...action,folder,format:'FLAC'});
    assert.ok(Math.abs(peak(await readFile(result.path))+1)<.02);
    assert.deepEqual(await readFile(source),original);
  }finally{await rm(folder,{recursive:true,force:true});}
});
