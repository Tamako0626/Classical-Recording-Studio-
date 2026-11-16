import {test} from 'node:test';
import assert from 'node:assert/strict';
import {ffmpegPaths} from '../desktop/ffmpeg-paths.mjs';

test('Finder installs resolve a complete Homebrew FFmpeg pair',()=>{
  const installed=new Set(['/usr/bin/ffmpeg','/opt/homebrew/bin/ffmpeg','/opt/homebrew/bin/ffprobe']);
  const canExecute=path=>installed.has(path);
  assert.deepEqual(ffmpegPaths({platform:'darwin',path:'/usr/bin:/bin',canExecute}),{
    ffmpeg:'/opt/homebrew/bin/ffmpeg',ffprobe:'/opt/homebrew/bin/ffprobe'
  });
  assert.deepEqual(ffmpegPaths({platform:'win32',canExecute}),{});
});
