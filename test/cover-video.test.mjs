import {test} from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdtemp,readFile,rm,readdir} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {createNativeProcessor} from '../desktop/native-ffmpeg.mjs';

const ff=args=>execFileSync('ffmpeg',['-hide_banner','-v','error',...args],{maxBuffer:16*1024*1024});
const probe=path=>JSON.parse(execFileSync('ffprobe',['-v','error','-show_streams','-show_format','-of','json',path]));
const hash=path=>ff(['-i',path,'-map','0:a:0','-c:a','pcm_s24le','-f','hash','-']).toString();
test('cover videos produce one seekable file per track and honor audio packaging modes',async()=>{
  const folder=await mkdtemp(join(tmpdir(),'studio-cover-video-'));
  try{
    const img=join(folder,'front.png'),high=join(folder,'01 - Allegro.flac'),low=join(folder,'02 - Andante.wav');
    ff(['-f','lavfi','-i','color=s=100x200:c=blue','-frames:v','1','-threads','1',img]);
    ff(['-f','lavfi','-i','sine=frequency=440:sample_rate=96000:duration=13.07','-c:a','flac','-sample_fmt','s32','-bits_per_raw_sample','24',high]);
    ff(['-f','lavfi','-i','sine=frequency=330:sample_rate=44100:duration=1.2',low]);
    const processor=createNativeProcessor();
    const params={cover:{name:'front.png',bytes:await readFile(img)},folder,sources:[high,low]};
    const original=await readFile(high);
    const batch=await processor.run('cover-videos',{...params,mode:'auto'});
    assert.deepEqual(batch.results.map(({target,error})=>[target,error]),[['01 - Allegro.mkv',undefined],['02 - Andante.mp4',undefined]]);
    const [mkv,mp4]=batch.results;
    assert.equal(mkv.rate,96000);
    assert.equal(mkv.bitDepth,24);
    assert.equal(mkv.codec,'flac');
    assert.equal(hash(mkv.path),hash(high));
    assert.ok(ff(["-ss","12.3","-i",mkv.path,"-map","0:v:0","-frames:v","1","-f","framehash","-"]).toString().includes("SHA256"));
    assert.equal(mp4.rate,44100);
    assert.equal(mp4.codec,'aac');
    for(const file of [mkv,mp4]){
      const streams=probe(file.path).streams;
      const video=streams.find(stream=>stream.codec_type==='video');
      assert.equal(video.disposition.attached_pic,0);
      assert.equal(video.width,1920);
      assert.equal(video.height,1080);
    }
    const upgrade=await processor.run('cover-videos',{...params,sources:[low],mode:'mkv48'});
    assert.deepEqual(upgrade.results.map(({rate,bitDepth,codec,sourceRate})=>[rate,bitDepth,codec,sourceRate]),[[48000,24,'pcm_s24le',44100]]);
    const repeated=await processor.run('cover-videos',{...params,sources:[low],mode:'mp4'});
    assert.match(repeated.results[0].error,/已有同名/);
    assert.deepEqual(await readFile(high),original);
    assert.ok((await readdir(folder)).every(name=>!name.startsWith('.studio-video-')));
  }finally{await rm(folder,{recursive:true,force:true});}
});
