import {app,dialog} from 'electron';
import {appendFileSync,mkdirSync} from 'node:fs';
import {join} from 'node:path';

function showStartupError(error){
  const report=error instanceof Error?error.stack||error.message:String(error);
  console.error(report);
  let log='';
  try{
    const folder=app.getPath('userData');
    mkdirSync(folder,{recursive:true});
    log=join(folder,'startup-error.log');
    appendFileSync(log,new Date().toISOString()+'\n'+report+'\n','utf8');
  }catch{}
  if(!process.argv.includes('--startup-check')){
    dialog.showErrorBox('古典录音工作台无法启动', (error.message||String(error))+(log?'\n\n诊断日志：'+log:''));
  }
  app.exit(1);
}

try{
  const {start}=await import('./main.mjs');
  // The entry module must finish evaluating before Electron emits ready.
  void start().catch(showStartupError);
}catch(error){showStartupError(error);}
