import {accessSync,constants} from 'node:fs';
import {posix} from 'node:path';

function executable(path){
  try{accessSync(path,constants.X_OK);return true;}
  catch{return false;}
}

export function ffmpegPaths({platform=process.platform,path=process.env.PATH||'',canExecute=executable}={}){
  if(platform!=='darwin')return {};
  const folders=new Set([...path.split(':'),'/opt/homebrew/bin','/usr/local/bin','/opt/local/bin']);
  for(const folder of folders){
    if(!folder)continue;
    const ffmpeg=posix.join(folder,'ffmpeg'),ffprobe=posix.join(folder,'ffprobe');
    if(canExecute(ffmpeg)&&canExecute(ffprobe))return {ffmpeg,ffprobe};
  }
  return {};
}
