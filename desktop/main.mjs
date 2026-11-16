import {videoAudioInput} from './cover-video.mjs';
import {app,BrowserWindow,dialog,ipcMain,Menu,protocol,safeStorage,session,shell} from 'electron';
import {constants,createReadStream,mkdirSync} from 'node:fs';
import {copyFile,mkdir,mkdtemp,rename,rm,stat,writeFile} from 'node:fs/promises';
import {basename,dirname,extname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID} from 'node:crypto';
import {Readable} from 'node:stream';
import {createLlm} from './llm.mjs';
import {ffmpegPaths} from './ffmpeg-paths.mjs';
import {openWorkspace} from './workspace.mjs';
import worker from '../dist/server/index.js';
import {albumOutputDirectory,createNativeProcessor} from './native-ffmpeg.mjs';
import {sourceValid} from '../web/script.js';

const origin='studio://local';
const assetOrigin='https://studio.local';
const videoFiles=new Map();
const files=new Map(),folders=new Map(),outputs=new Map(),albumCovers=new Map();
let window,projectMedia;
protocol.registerSchemesAsPrivileged([{scheme:'studio',privileges:{standard:true,secure:true,supportFetchAPI:true,stream:true,corsEnabled:true}}]);

const processor=createNativeProcessor({
  ...ffmpegPaths(),
  onProgress:seconds=>{if(window&&!window.isDestroyed())window.webContents.send('studio:progress','progress',seconds);}
});
function permitted(event){
  return !!window&&event.sender===window.webContents&&event.senderFrame===window.webContents.mainFrame&&event.senderFrame.url.startsWith(origin+'/');
}
function handle(channel,operation){
  ipcMain.handle('studio:'+channel,async(event,...args)=>{
    if(!permitted(event))return {error:'请求来源无效。'};
    try{return {value:await operation(...args)};}
    catch(error){return {error:error.message};}
  });
}
async function addAudio(path,name=basename(path),kind='audio'){
  if(!sourceValid(name))throw Error('录音文件名或格式无效：'+name);
  const info=await stat(path);
  if(!info.isFile()||!info.size)throw Error('录音文件为空或无法读取：'+name);
  const id=projectMedia.remember(kind,name,path);
  files.set(id,path);
  return {id,name,size:info.size,url:origin+'/media/'+id};
}
function sourcePath(id){
  const path=files.get(id);
  if(!path)throw Error('请重新导入录音文件。');
  return path;
}
async function mediaResponse(request,id){
  const path=files.get(id);
  if(!path)return new Response(null,{status:404});
  if(!['GET','HEAD'].includes(request.method))return new Response(null,{status:405});
  const {size}=await stat(path);
  const formats={'.wav':'audio/wav','.flac':'audio/flac','.mp3':'audio/mpeg','.mkv':'video/x-matroska','.ape':'audio/ape'};
  const headers={'Content-Type':formats[extname(path).toLowerCase()],'Accept-Ranges':'bytes'};
  const range=request.headers.get('range');
  let start=0,end=size-1;
  if(range){
    const match=range.match(/^bytes=(\d*)-(\d*)$/);
    if(!match||(!match[1]&&!match[2]))return new Response(null,{status:416,headers:{'Content-Range':'bytes */'+size}});
    if(match[1]){
      start=Number(match[1]);
      if(match[2])end=Math.min(end,Number(match[2]));
    }else start=Math.max(0,size-Number(match[2]));
    if(start>end||!Number.isSafeInteger(start)||!Number.isSafeInteger(end))return new Response(null,{status:416,headers:{'Content-Range':'bytes */'+size}});
    headers['Content-Range']='bytes '+start+'-'+end+'/'+size;
  }
  headers['Content-Length']=String(end-start+1);
  const body=request.method==='HEAD'?null:Readable.toWeb(createReadStream(path,{start,end}));
  return new Response(body,{status:range?206:200,headers});
}
export async function start(){
  const startupCheck=process.argv.includes('--startup-check');
  if(startupCheck&&process.env.STUDIO_STARTUP_CHECK_DIR){
    mkdirSync(process.env.STUDIO_STARTUP_CHECK_DIR,{recursive:true});
    app.setPath('userData',process.env.STUDIO_STARTUP_CHECK_DIR);
  }
  await app.whenReady();
  if(process.platform==='darwin')Menu.setApplicationMenu(Menu.buildFromTemplate([
    {role:'appMenu'},{role:'editMenu'},{role:'viewMenu'},{role:'windowMenu'}
  ]));
  const workspaceDir=app.getPath('userData');
  await mkdir(workspaceDir,{recursive:true});
  const env=openWorkspace(join(workspaceDir,'workspace.sqlite'));
  projectMedia=env.media;
  protocol.handle('studio',async request=>{
    try{
      const url=new URL(request.url);
      if(url.hostname!=='local')return new Response(null,{status:404});
      if(url.pathname.startsWith('/media/'))return await mediaResponse(request,url.pathname.slice(7));
      const headers=new Headers(request.headers);
      headers.set('oai-authenticated-user-id','desktop-user');
      if(headers.get('origin')===origin)headers.set('origin',assetOrigin);
      const options={method:request.method,headers};
      if(request.body){options.body=request.body;options.duplex='half';}
      return await worker.fetch(new Request(assetOrigin+url.pathname+url.search,options),env);
    }catch(error){console.error(error.message);return new Response('本机请求失败。',{status:500});}
  });

  const llm=createLlm(workspaceDir,safeStorage);
  handle('llm-get',llm.get);
  handle('llm-save',llm.save);
  handle('llm-test',llm.test);
  handle('llm-search-session',llm.searchSession);
  handle('llm-models',llm.models);
  handle('llm-recognize',llm.recognize);
  handle('save-workspace',async(body)=>{
    if(typeof body!=='string')throw Error('工作区数据无效。');
    const response=await worker.fetch(new Request(assetOrigin+'/api/workspace',{
      method:'PUT',headers:{origin:assetOrigin,'oai-authenticated-user-id':'desktop-user','content-type':'application/json'},body
    }),env);
    return {status:response.status,result:await response.json()};
  });
  handle('cd-artwork',async(bytes)=>{
    const {convertCdArtwork}=await import('./cd-artwork.mjs');
    return convertCdArtwork(bytes);
  });
  handle('audio',async expectedName=>{
    const relocating=typeof expectedName==='string';
    if(relocating&&!sourceValid(expectedName))throw Error('待重新关联的录音文件名无效。');
    const {canceled,filePaths}=await dialog.showOpenDialog(window,{title:relocating?'重新关联 '+expectedName:'导入录音',
      properties:relocating?['openFile']:['openFile','multiSelections'],filters:[{name:'录音文件',extensions:relocating?[extname(expectedName).slice(1).toLowerCase()]:['wav','flac','mp3','mkv','ape']}]});
    if(canceled)return [];
    if(relocating&&extname(filePaths[0]).toLowerCase()!==extname(expectedName).toLowerCase())throw Error('请选择与原录音格式相同的文件。');
    return Promise.all(filePaths.map(path=>addAudio(path,relocating?expectedName:basename(path))));
  });
  handle('remember-project-image',file=>projectMedia.saveImage(file));
  handle('restore-project-media',async request=>{
    const ids=request?.audio;
    if(!Array.isArray(ids)||ids.length>500||ids.some(id=>typeof id!=='string'||id.length>80))throw Error('项目音频引用无效。');
    const restore=async(id,kind)=>{
      const entry=projectMedia.get(id);
      if(entry?.kind!==kind)return null;
      try{return await addAudio(entry.path,entry.name,kind);}
      catch{return null;}
    };
    const [audio,merged,cover,backCover]=await Promise.all([
      Promise.all(ids.map(id=>restore(id,'audio'))),restore(request.merged,'merged'),projectMedia.loadImage(request.cover),projectMedia.loadImage(request.backCover)]);
    return {audio,merged,cover,backCover};
  });
  handle('video-sources',async()=>{
    const {canceled,filePaths}=await dialog.showOpenDialog(window,{title:'选择已经分轨的音频',properties:['openFile','multiSelections'],
      filters:[{name:'音频和媒体文件',extensions:['wav','flac','mp3','mkv','ape','m4a','aac','aif','aiff','wv','ogg','opus','mp4']}]});
    if(canceled)return [];
    return Promise.all(filePaths.map(async path=>{
      const info=await stat(path);
      const name=basename(path);
      if(!videoAudioInput.test(name)||!info.isFile()||!info.size)throw Error('无法使用音频：'+name);
      const id=randomUUID();videoFiles.set(id,path);
      return {id,name};
    }));
  });
  handle('folder',async()=>{
    const {canceled,filePaths}=await dialog.showOpenDialog(window,{title:'选择专辑文件夹的保存位置',properties:['openDirectory','createDirectory']});
    if(canceled)return null;
    const id=randomUUID();
    folders.set(id,filePaths[0]);
    return {id,name:filePaths[0]};
  });
  handle('album-folder',async(parentId,name,cover,backCover)=>{
    const parent=folders.get(parentId);
    if(!parent)throw Error('请先选择专辑保存位置。');
    const path=await albumOutputDirectory(parent,name);
    const id=randomUUID();
    const {coverPath,backPath}=cover||backCover
      ?await processor.run('archive-album-artwork',{folder:path,cover,backCover})
      :{};
    folders.set(id,path);
    if(coverPath)albumCovers.set(id,coverPath);
    return {id,name:path,coverName:coverPath?basename(coverPath):'',backCoverName:backPath?basename(backPath):''};
  });
  handle('process',async(action,payload)=>{
    if(action==='cover-videos'){
      const parent=folders.get(payload.folder);
      if(!parent)throw Error('请先选择视频保存位置。');
      if(!Array.isArray(payload.ids)||!payload.ids.length||payload.ids.length>500)throw Error('请选择 1 至 500 条音轨。');
      const sources=[...new Set(payload.ids)].map(id=>videoFiles.get(id)||outputs.get(id));
      if(sources.some(path=>!path))throw Error('有音轨需要重新选择。');
      const folder=await albumOutputDirectory(parent,'封面视频');
      const {results,stopped}=await processor.run(action,{sources:[...new Set(sources)],folder,mode:payload.mode,cover:payload.cover});
      return {folder,stopped,results:results.map(({path,...result})=>{
        if(!path)return result;
        const id=randomUUID();outputs.set(id,path);
        return {...result,id};
      })};
    }
    if(action==='concatenate'){
      const {ids,name}=payload;
      if(!Array.isArray(ids)||ids.length<2||ids.length>100||!sourceValid(name)||extname(name).toLowerCase()!=='.flac')throw Error('拼接文件列表或名称无效。');
      const mergedRoot=join(workspaceDir,'project-media','audio');
      await mkdir(mergedRoot,{recursive:true});
      const dir=await mkdtemp(join(mergedRoot,'merged-'));
      const output=join(dir,name);
      try{
        const info=await processor.run(action,{sources:ids.map(sourcePath),output});
        return {...info,file:await addAudio(output,name,'merged')};
      }catch(error){await rm(dir,{recursive:true,force:true});throw error;}
    }
    if(action==='preview-effects')return processor.run(action,{source:sourcePath(payload.id),job:payload.job,settings:payload.settings,segment:payload.segment});
    if(action==='preview')return processor.run(action,{source:sourcePath(payload.id),start:payload.start});
    if(action==='prepare'||action==='metadata')return processor.run(action,{source:sourcePath(payload.id)});
    if(action==='track'){
      const folder=folders.get(payload.folder);
      if(!folder)throw Error('请先选择输出目录。');
      const source=sourcePath(payload.id);
      const cover=albumCovers.get(payload.folder);
      const result=await processor.run(action,{source,cover,folder,job:payload.job,settings:payload.settings,format:payload.format});
      const id=randomUUID();
      outputs.set(id,result.path);
      return {id,duration:result.duration,note:'已保存到 '+folder};
    }
    throw Error('不支持此音频操作。');
  });
  handle('save-project',async(name,content)=>{
    if(typeof name!=='string'||basename(name)!==name||!name.endsWith('.crstudio.json')||
      typeof content!=='string'||Buffer.byteLength(content)>1800000)throw Error('项目文件的名称或大小无效。');
    const {canceled,filePath}=await dialog.showSaveDialog(window,{
      title:'保存录音项目',defaultPath:name,filters:[{name:'古典录音工作台项目',extensions:['json']}]
    });
    if(canceled||!filePath)return false;
    const staging=await mkdtemp(join(dirname(filePath),'.studio-project-'));
    try{
      const temporaryFile=join(staging,'project.json');
      await writeFile(temporaryFile,content,'utf8');
      await rename(temporaryFile,filePath);
      return true;
    }finally{await rm(staging,{recursive:true,force:true});}
  });
  handle('save-audio',async(id,name)=>{
    const path=sourcePath(id);
    if(projectMedia.get(id)?.kind!=='merged'||!sourceValid(name))throw Error('请先拼接录音。');
    const {canceled,filePath}=await dialog.showSaveDialog(window,{title:'保存拼接录音',defaultPath:name,filters:[{name:'FLAC',extensions:['flac']}]});
    if(canceled||!filePath)return;
    try{await copyFile(path,filePath,constants.COPYFILE_EXCL);}
    catch(error){if(error.code==='EEXIST')throw Error('目标文件已存在，请另选文件名。');throw error;}
  });
  handle('reveal',id=>{
    if(outputs.has(id))shell.showItemInFolder(outputs.get(id));
  });
  ipcMain.on('studio:stop',event=>{if(permitted(event))processor.cancel();});
  session.defaultSession.setPermissionRequestHandler((_contents,_permission,callback)=>callback(false));
  window=new BrowserWindow({show:!startupCheck,width:1380,height:920,minWidth:850,minHeight:560,webPreferences:{
    sandbox:true,contextIsolation:true,nodeIntegration:false,additionalArguments:startupCheck?['--studio-startup-check']:[],preload:fileURLToPath(new URL('./preload.cjs',import.meta.url))
  }});
  window.webContents.on('will-navigate',event=>event.preventDefault());
  window.webContents.on('will-attach-webview',event=>event.preventDefault());
  window.webContents.setWindowOpenHandler(({url})=>{
    if(/^https?:\/\//iu.test(url))void shell.openExternal(url);
    return {action:'deny'};
  });
  let closeReady=false,closeRequest='',closing=false;
  ipcMain.on('studio:close-ready',event=>{if(permitted(event))closeReady=true;});
  ipcMain.on('studio:close-answer',(event,id,allow)=>{
    if(!permitted(event)||!closeRequest||id!==closeRequest||typeof allow!=='boolean')return;
    closeRequest='';
    if(allow){closing=true;window.close();}
  });
  window.on('close',event=>{
    if(closing||!closeReady||window.webContents.isCrashed())return;
    event.preventDefault();
    if(closeRequest)return;
    closeRequest=randomUUID();
    window.webContents.send('studio:close-request',closeRequest);
  });
  if(startupCheck){
    // Wait for the renderer to load the workspace and mount the splitter.
    const ready=new Promise((resolve,reject)=>{
      const timeout=setTimeout(()=>reject(Error('分轨界面启动检查超时。')),15000);
      ipcMain.on('studio:startup-ready',event=>{
        if(!permitted(event))return;
        clearTimeout(timeout);resolve();
      });
    });
    await Promise.all([window.loadURL(origin+'/#splitter'),ready]);
  }
  app.on('window-all-closed',()=>app.quit());
  app.on('before-quit',event=>{
    if(window&&!window.isDestroyed()&&closeReady&&!closing&&!window.webContents.isCrashed()){
      event.preventDefault();
      window.close();
      return;
    }
    processor.cancel();
    env.close();
  });
  if(startupCheck)window.close();
  else await window.loadURL(origin+'/#splitter');
}
