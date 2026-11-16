const native=()=>window.studioNative;
const cloud='https://api.deepseek.com';
const presets=['deepseek-flash','deepseek-v4-pro'];
const modelLists=new Map();
const $=(root,selector)=>root.querySelector(selector);

async function call(action,...args){
  const {value,error}=await native()[action](...args);
  if(error)throw Error(error);
  return value;
}
export const llmConfig=()=>call('llmGet');
export const llmRecognize=(task,text,images,config)=>call('llmRecognize',{task,text,images,endpoint:config.base,model:config.model,vision:config.vision});

export async function openLlmSettings(){
  const dialog=document.createElement('dialog');
  dialog.className='program-dialog llm-settings';
  dialog.setAttribute('aria-label','LLM 配置');
  dialog.innerHTML=`<form>
    <div class="dialog-head"><h2>LLM 配置</h2><button type="button" class="button quiet" data-close>关闭</button></div>
    <div class="dialog-body">
      <div class="llm-setting"><label for="llm-provider">LLM 服务</label><select id="llm-provider" name="provider"><option value="deepseek">DeepSeek</option><option value="compatible">兼容 Chat Completions 的服务</option></select></div>
      <div class="llm-setting"><label for="llm-key">API Key<small>使用系统凭据加密</small></label><div><input id="llm-key" name="key" type="password" autocomplete="off" spellcheck="false"><label class="hint"><input type="checkbox" name="clearKey"> 清除已保存的 Key</label></div></div>
      <div class="llm-setting"><label for="llm-base">Base URL</label><input id="llm-base" name="base" type="url" required spellcheck="false"></div>
      <div class="llm-setting"><label for="llm-model-select">可选模型<small>从服务获取模型列表，也可手动输入 ID</small></label><div><div class="actions"><select id="llm-model-select" name="pickModel" aria-label="可选模型"></select><button id="llm-refresh" type="button" class="button">刷新模型列表</button></div><label class="field" data-custom-model><span>自定义模型 ID</span><input id="llm-model" name="model" spellcheck="false" maxlength="180"></label></div></div>
      <div class="llm-setting"><label for="llm-vision">图片识别<small>未启用时，图片先在本机 OCR</small></label><label><input id="llm-vision" name="vision" type="checkbox"> 模型支持图像输入</label></div>
      <div class="llm-setting"><label for="llm-test">连接测试<small>测试所选模型的文字请求</small></label><button id="llm-test" type="button" class="button">检查连接</button></div>
      <p class="hint" role="status" data-status></p>
    </div>
    <div class="dialog-foot"><button type="submit" class="button primary">保存配置</button></div>
  </form>`;
  document.body.append(dialog);
  const form=$(dialog,'form'),f=form.elements,say=msg=>{$(dialog,'[data-status]').textContent=msg;};
  const formData=()=>({provider:f.provider.value,base:f.base.value,model:f.model.value,key:f.key.value,clearKey:f.clearKey.checked,vision:f.vision.checked});
  const custom=$(dialog,'[data-custom-model]');
  const modelKey=()=>f.provider.value+'|'+f.base.value.trim().replace(/\/+$/u,'');
  let listedModels=[];
  function toggleCustom(){
    custom.hidden=f.pickModel.value!=='custom';
    f.model.required=!custom.hidden;
  }
  function renderModels(selected){
    const remote=modelLists.get(modelKey());
    listedModels=remote??(f.provider.value==='deepseek'?presets:[]);
    f.pickModel.replaceChildren(...listedModels.map((model,index)=>new Option(model,'model-'+index)),new Option('手动输入模型 ID','custom'));
    const index=listedModels.indexOf(selected);
    f.pickModel.value=index<0?'custom':'model-'+index;
    f.model.value=selected;
    toggleCustom();
  }
  function updateModel(){
    if(f.pickModel.value!=='custom')f.model.value=listedModels[Number(f.pickModel.value.slice(6))];
    toggleCustom();
  }
  f.pickModel.onchange=updateModel;
  f.base.onchange=()=>renderModels(f.model.value);
  function fill(value){
    for(const key of ['provider','base'])f[key].value=value[key];
    renderModels(value.model);
    f.vision.checked=value.vision;
    f.key.value='';
    f.key.placeholder=value.hasKey?'已保存 · 留空继续使用':'输入服务商提供的 API Key';
    f.clearKey.checked=false;
  }
  function lock(busy){for(const item of form.elements)if(!item.hasAttribute('data-close'))item.disabled=busy;}
  async function run(action){
    const config=formData();lock(true);
    try{await action(config);}catch(error){say(error.message);}finally{lock(false);}
  }
  f.provider.onchange=()=>{
    f.base.value=f.provider.value==='deepseek'?cloud:'';
    f.model.value=f.provider.value==='deepseek'?'deepseek-flash':'';
    f.vision.checked=false;
    f.key.value='';
    f.key.placeholder='更改服务时请输入对应的 API Key';
    renderModels(f.model.value);
    say('');
  };
  $(dialog,'[data-close]').onclick=()=>dialog.close();
  function publishModels(models,selected){
    modelLists.set(modelKey(),models);
    renderModels(selected||models[0]||'');
  }
  $(dialog,'#llm-refresh').onclick=()=>run(async config=>{
    say('正在读取模型列表…');
    const models=await call('llmModels',config);
    publishModels(models,config.model);
    say(models.length?'已读取 '+models.length+' 个模型，可从下拉列表选择。':'该服务未列出模型，可手动输入模型 ID。');
  });
  $(dialog,'#llm-test').onclick=()=>run(async config=>{
    if(!config.model.trim()){say('请先选择模型，或点击「刷新模型列表」。');return;}
    say('正在检查连接…');
    const result=await call('llmTest',config);
    if(result.models.length)publishModels(result.models,config.model);
    say(result.chat?'模型连接成功。':result.chatError||'请选择模型后再次测试。');
  });
  form.onsubmit=event=>{event.preventDefault();void run(async config=>{
    const previous=await llmConfig();
    if(previous.hasKey&&previous.base===config.base&&config.clearKey&&config.key.trim())throw Error('清除已保存的 Key 时请留空。');
    fill(await call('llmSave',config));
    say('配置已保存，可在导入窗口使用模型识别。');
  });};
  dialog.addEventListener('close',()=>dialog.remove(),{once:true});
  lock(true);
  dialog.showModal();
  try{fill(await llmConfig());}catch(error){say(error.message);}finally{lock(false);}
}

export function attachWorkModel(button,status,getQuery,task,show){
  let config;
  button.disabled=true;
  void llmConfig().then(value=>{
    if(!button.isConnected)return;
    if(!value.model||value.provider==='deepseek'&&!value.hasKey){
      status.textContent='在右上角保存 LLM 配置后可使用模型建议。';
      return;
    }
    config=value;
    button.disabled=false;
    status.textContent='使用 '+config.model+' 时，只发送查询内容和作曲家到 '+new URL(config.base).host+'。';
  }).catch(error=>{status.textContent=error.message;});
  button.onclick=async()=>{
    const request=getQuery();
    if(!request.title){status.textContent='请先填写作品名称或编号。';return;}
    button.disabled=true;
    status.textContent='正在向 '+config.model+' 请求建议…';
    try{
      const text=await llmRecognize(task,JSON.stringify(request),[],config);
      if(!button.isConnected)return;
      const raw=text.replace(/^```(?:json)?\s*|\s*```$/giu,'');
      let result;
      try{result=JSON.parse(raw);}catch{throw Error('模型回复格式无效，请重试。');}
      if(JSON.stringify(getQuery())!==JSON.stringify(request)){
        status.textContent='查询已更改。可按新内容重新匹配。';return;
      }
      status.textContent=show(result);
    }catch(error){if(button.isConnected)status.textContent=error.message;}
    finally{if(button.isConnected)button.disabled=false;}
  };
}
