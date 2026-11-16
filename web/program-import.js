import {openLlmFiles} from './llm-files.js';
import {isProgramJunk,parseProgram,parseSessionText} from './program-parser.js';
import {timeSeconds} from './script.js';

const fileTypes=['image/png','image/jpeg','image/webp','image/bmp','image/gif'];
const imageExtension=/\.(png|jpe?g|webp|bmp|gif)$/i;
const html=value=>String(value??'').replace(/[&<>"']/gu,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const limit=50000;

export async function recognizePages(pages,report,signal){
  const {default:{createWorker}}=await import('/program-ocr.js');
  if(signal.aborted)return;
  let worker, rejectOperation, timer, currentPage;
  let finished=false;
  const terminate=()=>{if(worker)void worker.terminate().catch(()=>{});};
  const abort=()=>{terminate();rejectOperation?.(signal.reason);};
  signal.addEventListener('abort',abort,{once:true});
  function run(operation){
    const failed=new Promise((_,reject)=>{
      rejectOperation=reject;
      timer=setTimeout(()=>{
        terminate();
        reject(Object.assign(Error('图片识别超时，请重试。'),{timeout:true}));
      },120000);
    });
    return Promise.race([Promise.resolve().then(operation),failed]).finally(()=>clearTimeout(timer));
  }
  try{
    await run(async ()=>{
      worker=await createWorker(['chi_sim','eng'],1,{
        workerPath:location.origin+'/program-ocr-worker.js',
        corePath:location.origin+'/program-ocr-core.js',
        workerBlobURL:false,
        logger:({status,progress})=>{
          if(!finished&&!signal.aborted)report(currentPage,status==='recognizing text'?Math.round(progress*100):null);
        },
        errorHandler:error=>rejectOperation?.(Error(String(error)))
      });
      if(finished||signal.aborted){terminate();return;}
      await worker.setParameters({tessedit_pageseg_mode:'3',preserve_interword_spaces:'0'});
    });
    for(const page of pages){
      if(signal.aborted)return;
      currentPage=page;
      report(page,0);
      try{
        const {data}=await run(()=>worker.recognize(page.file));
        if(signal.aborted)return;
        page.text=data.text.trim();
        page.error=page.text?'':'未识别到文字';
      }catch(error){
        if(signal.aborted)return;
        page.error=error.timeout?'识别超时':'无法识别此图片';
        if(error.timeout)throw error;
      }
    }
  }finally{
    finished=true;
    clearTimeout(timer);
    signal.removeEventListener('abort',abort);
    terminate();
  }
}

export function openProgramImport({fields,existingCount,apply,sessionOnly=false,modelFiles=[],initialSession={}}){
  const dialog=document.createElement('dialog');
  dialog.className='program-dialog';
  dialog.setAttribute('aria-labelledby','program-import-title');
  const title=sessionOnly?'识别场次信息':'导入节目单';
  const example=sessionOnly?'音乐会：钢琴独奏会&#10;演奏家：Michel Dalberto&#10;日期：2026-11-06&#10;音乐厅：上海交响音乐厅':'场次：钢琴独奏会&#10;日期：2026-11-06&#10;曲目：&#10;1. 莫扎特：A 大调钢琴奏鸣曲 K. 331';
  const canSearch=sessionOnly&&window.studioNative?.llmSearchSession;
  dialog.innerHTML='<div class="dialog-head"><h2 id="program-import-title">'+title+'</h2><button type="button" class="button quiet" data-close>关闭</button></div>'+
    '<div class="dialog-body"><div class="form-grid"><label class="field wide"><span>上传图片（可多选）或 TXT 文件</span><input type="file" data-file multiple accept=".png,.jpg,.jpeg,.webp,.bmp,.gif,.txt,text/plain"></label>'+
    '<div class="wide" data-pages hidden></div><label class="field wide"><span>输入或粘贴文字，也可校对图片识别原文</span><textarea data-program-text rows="6" maxlength="'+limit+'" placeholder="'+example+'"></textarea></label></div>'+
    (window.studioNative?.llmGet?'<div class="actions section-gap"><button class="button" type="button" data-llm>用模型识别图片或文件</button></div>':'')+
    '<p class="hint">多张图片会按页序合并，可调整页序或补选图片。图片在本机浏览器识别。'+(sessionOnly?'':'未提供的分轨时间保持空白。')+'</p>'+
    '<p role="status" class="hint" data-status>选择文件或粘贴文字后自动识别。</p><div data-parsed></div>'+
    (canSearch?'<section><div class="actions"><button class="button" type="button" data-session-search>联网核对</button><span class="hint">查询会将当前场次字段发送给 DeepSeek。</span></div><p class="hint" role="status" data-search-status></p><div data-session-candidates></div></section>':'')+'</div>'+
    '<div class="dialog-foot"><button type="button" class="button" data-cancel>取消</button><button type="button" class="button primary" data-apply disabled>'+ (sessionOnly?'填入场次信息':'填入分轨页')+'</button></div>';
  document.body.append(dialog);
  const $=selector=>dialog.querySelector(selector);
  const source=$('[data-program-text]'), result=$('[data-parsed]'), status=$('[data-status]'), submit=$('[data-apply]');
  let jobs, timer, parsed;
  let pages=[], busy=false, editedText=false;
  const picker=$('[data-file]');
  const pageList=$('[data-pages]');
  const setStatus=message=>{status.textContent=message;};
  const searchButton=$('[data-session-search]');
  const searchStatus=$('[data-search-status]');
  const candidates=$('[data-session-candidates]');
  let searchVersion=0;
  const sessionValues=()=>Object.fromEntries([...result.querySelectorAll('[data-session]')].flatMap(input=>{
    const value=input.value.trim();
    return value?[[input.dataset.session,value]]:[];
  }));
  function resetSearch(){
    searchVersion++;
    if(!searchButton)return;
    searchButton.disabled=false;
    searchStatus.textContent='';
    candidates.replaceChildren();
  }
  if(searchButton)searchButton.onclick=async()=>{
    if(busy)return;
    const {origin,...clues}=sessionValues();
    if(!Object.keys(clues).length){searchStatus.textContent='请填写演奏家、日期、音乐厅等已知场次信息。';return;}
    const version=++searchVersion;
    searchButton.disabled=true;
    candidates.replaceChildren();
    searchStatus.textContent='正在搜索演出来源…';
    try{
      const {value,error}=await window.studioNative.llmSearchSession(clues);
      if(!dialog.open||version!==searchVersion)return;
      if(error)throw Error(error);
      searchStatus.textContent=value.length?'找到 '+value.length+' 个候选。请核对演出日期、地点和来源。':'没有找到有可核对来源的场次。可补充日期或场馆后重试。';
      for(const candidate of value){
        const card=document.createElement('article');
        card.className='work-candidate session-candidate';
        const title=candidate.session.name||[candidate.session.performer,candidate.session.date,candidate.session.venue].filter(Boolean).join(' · ')||'场次候选';
        card.innerHTML='<strong>'+html(title)+'</strong><dl>'+Object.entries(candidate.session).map(([key,content])=>
          '<div><dt>'+html(fields[key])+'</dt><dd>'+html(content)+(clues[key]&&clues[key]!==content?' <small>当前：'+html(clues[key])+'</small>':'')+'</dd></div>').join('')+'</dl><div class="actions"></div>';
        const actions=card.lastElementChild;
        const use=document.createElement('button');
        use.type='button';use.className='button';use.textContent='使用此场次';
        use.onclick=()=>{
          for(const [key,content] of Object.entries(candidate.session)){
            const input=result.querySelector('[data-session="'+key+'"]');
            if(input)input.value=content;
          }
          submit.disabled=false;
          searchStatus.textContent='候选已填入校对表。若要替换分轨页已有值，请勾选「用校对值替换分轨页已填写的场次字段」。';
        };
        actions.append(use);
        for(const source of candidate.sources){
          const link=document.createElement('a');
          link.className='work-source';link.href=source.url;link.target='_blank';link.rel='noopener noreferrer';
          link.textContent=source.title+' ↗';
          actions.append(link);
        }
        candidates.append(card);
      }
    }catch(error){if(dialog.open&&version===searchVersion)searchStatus.textContent=error.message;}
    finally{if(dialog.open&&version===searchVersion)searchButton.disabled=false;}
  };
  function render(){
    if(sessionOnly){
      resetSearch();
      const recognized={...initialSession,...parseSessionText(source.value)};
      const count=Object.values(recognized).filter(Boolean).length;
      submit.disabled=!count;
      setStatus(count?'已识别 '+count+' 项场次信息，请核对后填入。':source.value.trim()?'未找到明确的场次字段。可补充字段标签或直接填写下方表格。':'上传图片、TXT 或输入文字后自动识别。');
      result.innerHTML='<section><h3>核对场次信息</h3><div class="form-grid">'+Object.entries(fields).map(([key,label])=>
        '<label class="field span2"><span>'+html(label)+'</span><input data-session="'+key+'" type="'+(key==='date'?'date':'text')+'" maxlength="280" value="'+html(recognized[key])+'"></label>'
      ).join('')+'</div><p class="hint"><label><input type="checkbox" data-overwrite> 用校对值替换分轨页已填写的场次字段</label></p></section>';
      return;
    }
    parsed=parseProgram(source.value);
    const found=Object.keys(parsed.session).length+parsed.tracks.length;
    submit.disabled=!found;
    setStatus(found?'已识别 '+Object.keys(parsed.session).length+' 项场次信息和 '+parsed.tracks.length+' 首曲目。'+(parsed.merged?'已合并 '+parsed.merged+' 条重复曲目，采用原文或英文标题。':'')+'请核对后填入。':source.value.trim()?'未识别到明确的场次或曲目。可给场次字段加标签，或在曲目前加序号。':'选择文件或粘贴文字后自动识别。');
    if(!source.value.trim()){result.replaceChildren();return;}
    const cleanup=[];
    const translated=parsed.tracks.filter(track=>track.englishOriginal).length;
    const unverified=parsed.tracks.filter(track=>track.englishReview).length;
    if(translated)cleanup.push(translated+' 首已匹配英文曲名');
    if(unverified)cleanup.push(unverified+' 首中文曲名待核对');
    if(parsed.corrected)cleanup.push('已按曲目目录校对 '+parsed.corrected+' 处作曲家或作品号');
    if(parsed.excluded.length)cleanup.push('已排除 '+parsed.excluded.length+' 行杂项');
    if(parsed.review.length)cleanup.push(parsed.review.length+' 行待核对');
    if(cleanup.length)setStatus(status.textContent+' '+cleanup.join('，')+'。');
    const session=Object.keys(parsed.session).length?'<div class="form-grid">'+Object.entries(parsed.session).map(([key,value])=>
      '<label class="field span2"><span>'+html(fields[key])+'</span><input data-session="'+key+'" type="'+(key==='date'?'date':'text')+'" maxlength="280" value="'+html(value)+'"></label>'
    ).join('')+'</div>':'<p class="hint">未找到场次字段。</p>';
    const table=parsed.tracks.length?'<div class="table-wrap program-table"><table><thead><tr><th scope="col">导入</th><th scope="col">曲目名</th><th scope="col">作曲家</th><th scope="col">起点</th><th scope="col">终点</th></tr></thead><tbody>'+
      parsed.tracks.map((track,i)=>'<tr data-program-row><td><input type="checkbox" aria-label="导入第 '+(i+1)+' 轨" checked></td>'+
        ['title','composer','start','end'].map(key=>'<td><input data-program-field="'+key+'" aria-label="第 '+(i+1)+' 轨'+({title:'曲目名',composer:'作曲家',start:'起点',end:'终点'}[key])+'" maxlength="280" value="'+html(track[key])+'">'+(key==='title'&&track.englishOriginal?'<small class="split-filename">原文：'+html(track.englishOriginal)+'</small>':'')+(key==='title'&&track.englishReview?'<small class="split-filename">待核对英文曲名</small>':'')+'</td>').join('')+'</tr>').join('')+
      '</tbody></table></div>':'<p class="hint">未找到曲目。</p>';
    result.innerHTML='<section><h3>场次信息</h3>'+session+(Object.keys(parsed.session).length?'<p class="hint"><label><input type="checkbox" data-overwrite> 用导入值替换已填场次字段</label></p>':'')+'</section>'+
      '<section><h3>曲目预览</h3>'+table+(existingCount&&parsed.tracks.length?'<p class="hint"><label><input type="checkbox" data-replace> 替换现有 '+existingCount+' 条曲目（默认追加）</label></p>':'')+'</section>'+
      (parsed.review.length?'<details><summary>待核对文字（'+parsed.review.length+' 行）</summary><p class="hint">如果其中有曲目，可在上方原文中补全曲名，或在行首加序号，例如「1. 作品名」。</p><pre>'+html(parsed.review.join('\n'))+'</pre></details>':'')+
      (parsed.unassigned.length?'<details><summary>未分类文字（'+parsed.unassigned.length+' 行）</summary><pre>'+html(parsed.unassigned.join('\n'))+'</pre></details>':'');
  }
  function showPages(){
    pageList.hidden=!pages.length;
    pageList.innerHTML=pages.length?'<div class="program-page-head"><strong>节目单页序</strong><span class="hint">共 '+pages.length+' 页</span></div><ol class="program-pages">'+pages.map((page,index)=>
      '<li><span class="program-page-label" title="'+html(page.file.name)+'">'+html(page.file.name)+'</span><span class="hint">'+html(page.text?'已识别':page.error||'待识别')+'</span>'+
      '<div class="actions">'+[['up','上移',index===0],['down','下移',index===pages.length-1],['remove','移除',false]].map(([action,label,edge])=>
        '<button type="button" class="button quiet" data-page="'+index+'" data-move="'+action+'" aria-label="'+label+'第 '+(index+1)+' 页"'+(busy||edge?' disabled':'')+'>'+label+'</button>'
      ).join('')+'</div></li>'
    ).join('')+'</ol>'+(pages.some(p=>!p.text)?'<button class="button" type="button" data-retry'+(busy?' disabled':'')+'>重新识别未完成页</button>':''):'';
  }
  function showMerged(){
    const merged=pages.map(page=>page.text).filter(Boolean).join('\n\n');
    source.value=merged.slice(0,limit);
    editedText=false;
    render();
    const issues=[];
    const missing=pages.filter(page=>!page.text).length;
    if(missing)issues.push(missing+' 页未识别，可重试或粘贴缺少的文字。');
    if(merged.length>limit)issues.push('内容只保留前 '+limit+' 个字符。');
    if(issues.length)setStatus(status.textContent+' '+issues.join(' '));
  }
  function allowRebuild(){
    if(!editedText)return true;
    return window.confirm('重新合并图片会重新生成识别原文。要继续吗？');
  }
  async function readImages(){
    const pending=pages.filter(page=>!page.text);
    if(!pending.length){showMerged();return;}
    for(const page of pending)page.error='';
    const current=new AbortController();
    jobs=current;
    resetSearch();
    busy=true;
    picker.disabled=true;
    submit.disabled=true;
    source.readOnly=true;
    showPages();
    setStatus('正在准备图片识别…');
    try{
      await recognizePages(pending,(page,progress)=>{
        if(current.signal.aborted)return;
        if(page){
          const number=pages.indexOf(page)+1;
          setStatus('正在识别第 '+number+' / '+pages.length+' 页'+(progress===null?'…':'… '+progress+'%'));
        }else setStatus('正在准备中英文识别引擎…');
      },current.signal);
    }catch(error){
      if(current.signal.aborted)return;
      for(const page of pending)if(!page.text&&!page.error)page.error='等待重试';
      showMerged();
      setStatus(status.textContent+' '+(error?.message||'图片识别失败。'));
      return;
    }finally{
      if(!current.signal.aborted){
        busy=false;
        picker.disabled=false;
        source.readOnly=false;
        showPages();
      }
    }
    if(!current.signal.aborted)showMerged();
  }
  pageList.addEventListener('click',event=>{
    const button=event.target.closest('button');
    if(!button||busy||!allowRebuild())return;
    if(button.hasAttribute('data-retry')){void readImages();return;}
    const i=Number(button.dataset.page);
    const action=button.dataset.move;
    if(action==='remove')pages.splice(i,1);
    if(action==='up'||action==='down'){
      const next=i+(action==='up'?-1:1);
      [pages[i],pages[next]]=[pages[next],pages[i]];
    }
    showPages();
    showMerged();
    const focusIndex=action==='remove'?Math.min(i,pages.length-1):i+(action==='up'?-1:1);
    const row=pageList.querySelectorAll('li')[focusIndex];
    (row?.querySelector('button[data-move="'+action+'"]:enabled')||row?.querySelector('button:enabled')||picker).focus();
  });
  function recognizeWithModel(){
    if(busy)return;
    void openLlmFiles({
      task:sessionOnly?'session':'program',files:[...modelFiles,...pages.map(page=>page.file)],text:editedText||!pages.length?source.value:'',
      apply:result=>{
        clearTimeout(timer);
        source.value=result;
        editedText=true;
        render();
      }
    });
  }
  if($('[data-llm]'))$('[data-llm]').onclick=recognizeWithModel;
  $('[data-close]').onclick=()=>dialog.close();
  $('[data-cancel]').onclick=()=>dialog.close();
  dialog.addEventListener('close',()=>{clearTimeout(timer);jobs?.abort();dialog.remove();},{once:true});
  source.addEventListener('input',()=>{
    editedText=true;
    resetSearch();
    clearTimeout(timer);
    submit.disabled=true;
    result.replaceChildren();
    timer=setTimeout(render,120);
  });
  result.addEventListener('input',event=>{
    if(!sessionOnly||!event.target.hasAttribute('data-session'))return;
    resetSearch();
    submit.disabled=busy||!Object.keys(sessionValues()).length;
  });
  picker.addEventListener('change',async ()=>{
    if(busy)return;
    const files=[...picker.files];
    picker.value='';
    if(!files.length)return;
    const textFiles=files.filter(file=>/\.txt$/i.test(file.name)||file.type==='text/plain');
    if(textFiles.length&&files.length!==1){setStatus('请单独选择一个 TXT，或一次选择多张图片。');return;}
    if(textFiles.length){
      if(files[0].size>256000){setStatus('TXT 文件超过 256 KB。');return;}
      if(!allowRebuild())return;
      clearTimeout(timer);
      resetSearch();
      busy=true;
      picker.disabled=true;
      source.readOnly=true;
      submit.disabled=true;
      setStatus('正在读取文字…');
      try{
        const text=await files[0].text();
        if(!dialog.open)return;
        if(!text.trim())throw Error('TXT 没有可识别的文字。');
        pages=[];
        editedText=false;
        source.value=text.slice(0,limit);
        showPages();
        render();
        if(text.length>limit)setStatus(status.textContent+' 内容只保留前 '+limit+' 个字符。');
      }catch(error){
        if(!dialog.open)return;
        submit.disabled=sessionOnly
          ? ![...result.querySelectorAll('[data-session]')].some(input=>input.value.trim())
          : !parsed||!Object.keys(parsed.session).length&&!parsed.tracks.length;
        setStatus(error.message);
      }finally{
        busy=false;
        picker.disabled=false;
        source.readOnly=false;
      }
      return;
    }
    if(files.some(file=>!fileTypes.includes(file.type)&&!(file.type===''&&imageExtension.test(file.name)))){
      setStatus('请选择 PNG、JPG、WebP、BMP 或 GIF 图片。');return;
    }
    if(files.some(file=>file.size>20000000)){setStatus('每张图片须小于 20 MB。');return;}
    if(!allowRebuild())return;
    clearTimeout(timer);
    pages.push(...files.map(file=>({file,text:'',error:''})));
    await readImages();
  });
  submit.onclick=()=>{
    const session=sessionValues();
    const tracks=[...result.querySelectorAll('[data-program-row]')].filter(row=>row.querySelector('input[type=checkbox]').checked).map(row=>
      Object.fromEntries([...row.querySelectorAll('[data-program-field]')].map(input=>[input.dataset.programField,input.value.trim()]))
    ).filter(track=>track.title);
    if(!tracks.length&&!Object.keys(session).length){setStatus('请选中要导入的曲目，或填写场次信息。');return;}
    try{
      tracks.forEach(({title,start,end},index)=>{
        if(isProgramJunk(title))throw Error('所选第 '+(index+1)+' 行缺少可用的曲名，请修正或取消勾选。');
        const from=timeSeconds(start),to=timeSeconds(end);
        if(Number.isNaN(from)||Number.isNaN(to)||(from!==null&&to!==null&&to<=from))throw Error('请核对所选第 '+(index+1)+' 首曲目的时间。');
      });
      apply({session,tracks,overwrite:!!$('[data-overwrite]')?.checked,replace:!!$('[data-replace]')?.checked});
      dialog.close();
    }catch(error){setStatus(error.message);}
  };
  if(sessionOnly)render();
  dialog.showModal();
  if(modelFiles.length)recognizeWithModel();
}
