const {contextBridge,ipcRenderer}=require('electron');
contextBridge.exposeInMainWorld('studioNative',{
  startupCheck:process.argv.includes('--studio-startup-check'),
  saveWorkspace:body=>ipcRenderer.invoke('studio:save-workspace',body),
  onCloseRequest:listener=>{
    ipcRenderer.on('studio:close-request',(_event,id)=>listener(id));
    ipcRenderer.send('studio:close-ready');
  },
  answerClose:(id,allow)=>ipcRenderer.send('studio:close-answer',id,allow),
  startupReady:()=>ipcRenderer.send('studio:startup-ready'),
  llmGet:()=>ipcRenderer.invoke('studio:llm-get'),
  llmSave:config=>ipcRenderer.invoke('studio:llm-save',config),
  llmModels:config=>ipcRenderer.invoke('studio:llm-models',config),
  llmSearchSession:clues=>ipcRenderer.invoke('studio:llm-search-session',clues),
  llmTest:config=>ipcRenderer.invoke('studio:llm-test',config),
  llmRecognize:payload=>ipcRenderer.invoke('studio:llm-recognize',payload),
  convertCdArtwork:bytes=>ipcRenderer.invoke('studio:cd-artwork',bytes),
  selectVideoSources:()=>ipcRenderer.invoke('studio:video-sources'),
  selectAudio:expectedName=>ipcRenderer.invoke('studio:audio',expectedName),
  rememberProjectImage:file=>ipcRenderer.invoke('studio:remember-project-image',file),
  restoreProjectMedia:refs=>ipcRenderer.invoke('studio:restore-project-media',refs),
  selectFolder:()=>ipcRenderer.invoke('studio:folder'),
  prepareAlbumFolder:(parent,name,cover,backCover)=>ipcRenderer.invoke('studio:album-folder',parent,name,cover,backCover),
  process:(action,payload)=>ipcRenderer.invoke('studio:process',action,payload),
  stop:()=>ipcRenderer.send('studio:stop'),
  saveProjectFile:(name,content)=>ipcRenderer.invoke('studio:save-project',name,content),
  saveAudio:(id,name)=>ipcRenderer.invoke('studio:save-audio',id,name),
  reveal:id=>ipcRenderer.invoke('studio:reveal',id),
  onProgress:listener=>{
    const receive=(_event,kind,seconds)=>listener(kind,seconds);
    ipcRenderer.on('studio:progress',receive);
    return ()=>ipcRenderer.removeListener('studio:progress',receive);
  }
});
