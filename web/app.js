import {effectsEnabled,effectsForm,effectsValid,limiterForm,readEffects} from './audio-effects.js';
import {openLlmSettings} from './llm-settings.js';
import {openLlmFiles} from './llm-files.js';
import {createSplitter} from './splitter.js';
import {sessionFolderName} from './session-english.js';
import {seed, blankProject, blankTrack, statuses, tags, operations, defaults, naming} from './seed.js';
import {makePlan,renderScript,sourceValid,coverValid,outputValid,timeSeconds,filenameFor} from './script.js';

const $ = query=>document.querySelector(query);
const main=$('#main'), dialog=$('#dialog'), file=$('#file-input');
const labels={projects:'录音项目',splitter:'整场录音分轨',scripts:'脚本生成',history:'处理记录',templates:'模板与命名'};
const fields={performer:'演奏家',conductor:'指挥家',composer:'作曲家',date:'演出日期',city:'城市',venue:'音乐厅',origin:'录音来源',status:'项目状态'};
const coverStates=['待准备','待核对','已准备','已嵌入','需修复','无需封面'];
const outcomes=['成功','失败','跳过','待核对','已记录'];
const stepNames={split:'分轨',rename:'重命名',tags:'重写标签',cover:'嵌入封面',gain:'增益调整',width:'声场处理',reverb:'混响处理',effects:'高级后期处理',format:'格式转换',check:'质量检查'};
let splitter;
let splitExisting;
let data,revision=0,dirty=false,saving=null,blocked=false,change=0,toastTimer,lastSaveError='',discardOnExit=false;
let projectId='',projectTab='overview',scriptProject='',scriptState=null,generated=null,historyProject='',historyOperation='',historyOutcome='',activeGroup='',moreFilters=false;
const projectSorts=[['recent','日期从近到远'],['oldest','日期从远到近'],['name','项目名称'],['manual','手动顺序']];
const currentSort=()=>projectSorts.some(([key])=>key===data?.projectSort)?data.projectSort:'recent';
const filters={q:'',status:'',origin:'',date:'',performer:'',conductor:'',composer:'',city:'',venue:''};
const esc=value=>String(value ?? '').replace(/[&<>"']/g,char=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
const val=esc;
const unique=items=>[...new Set(items.filter(Boolean))].sort((a,b)=>a.localeCompare(b,'zh-CN'));
const pBy=id=>data.projects.find(p=>p.id===id);
const options=(values,current)=>values.map(entry=>{const [v,l]=Array.isArray(entry)?entry:[entry,entry];return '<option value="'+val(v)+'"'+(String(v)===String(current)?' selected':'')+'>'+esc(l)+'</option>';}).join('');
const select=(name,values,current)=>'<select name="'+name+'">'+options(values,current)+'</select>';
const input=(name,value='',attr='')=>'<input name="'+name+'" value="'+val(value)+'" '+attr+'>';
const area=(name,value='',rows=4,attr='')=>'<textarea name="'+name+'" rows="'+rows+'" '+attr+'>'+esc(value)+'</textarea>';
const field=(label,control,css='')=>'<label class="field '+css+'"><span>'+label+'</span>'+control+'</label>';
const button=(label,action,style='',id='')=>'<button type="button" class="button '+style+'" data-action="'+action+'" data-id="'+val(id)+'">'+label+'</button>';
const badge=s=>'<span class="badge '+(['需修复','处理失败','失败'].includes(s)?'red':['已完成','成功','已嵌入','已准备'].includes(s)?'green':['待核对时间点','待核对'].includes(s)?'amber':['待整理','跳过'].includes(s)?'gray':'')+'">'+esc(s)+'</span>';
const head=(title,description='',actions='')=>'<div class="page-head"><div><h1>'+esc(title)+'</h1>'+(description?'<p>'+esc(description)+'</p>':'')+'</div><div class="actions">'+actions+'</div></div>';
const table=(headers,body)=>'<div class="table-wrap"><table><thead><tr>'+headers.map(h=>'<th scope="col">'+h+'</th>').join('')+'</tr></thead><tbody>'+body+'</tbody></table></div>';
const tr=cells=>'<tr>'+cells.map(c=>'<td>'+c+'</td>').join('')+'</tr>';
const dateTime=time=>new Date(time).toLocaleString('zh-CN',{hour12:false});
const getForm=form=>Object.fromEntries(new FormData(form));
function toast(message) {
  const el=$('#toast');el.textContent=message;el.classList.add('show');clearTimeout(toastTimer);toastTimer=setTimeout(()=>el.classList.remove('show'),4400);
}
function sync(message){$('#sync').textContent=message;}
function localRecovery(){try{localStorage.setItem('classical-studio-recovery',JSON.stringify({revision,data}));}catch{}}
function persist() {
  dirty=true; change++; localRecovery(); void save();
}
async function save() {
  if(saving)return saving;
  if(!dirty||blocked)return;
  saving=writeWorkspace();
  try{await saving;}
  finally{saving=null;}
}
async function writeWorkspace(){
  lastSaveError='';
  try{
    while(dirty&&!blocked){
      const snapshot=change;
      sync('正在保存…');
      const body=JSON.stringify({revision,data});
      const {value,error}=await window.studioNative.saveWorkspace(body);
      if(error)throw Error(error);
      const {status,result}=value;
      if(status!==200){
        if(status===409)blocked=true;
        throw Error(result.error||'保存失败');
      }
      revision=result.revision;dirty=snapshot!==change;
      if(dirty) localRecovery();
      if(!dirty) {try{localStorage.removeItem('classical-studio-recovery');}catch{}}
      sync('已保存到工作区');
    }
  }catch(error){lastSaveError=error.message;sync('未同步 · 已尝试保存本机草稿');toast(error.message+'，可导出备份。');}
}
function addLog(p,operation,result,message,extra={}){
  data.logs.unshift({id:crypto.randomUUID(),projectId:p?.id||'',projectName:p?.name||'',operation,result,message,time:new Date().toISOString(),...extra});
}
function changeLog(p,message){addLog(p,'修订记录','已记录',message);}
function modify(callback){callback();persist();render();}
function saveSplitProject(project,rule){
  const existing=pBy(project.id);
  const previousParts=JSON.stringify(existing?.recordingParts);
  const updated={...project,namingRule:rule,status:existing?.status||'待核对时间点'};
  if(existing){
    if(existing.coverStatus==='已嵌入'&&existing.coverFile===updated.coverFile)updated.coverStatus='已嵌入';
    Object.assign(existing,updated);
    changeLog(existing,'更新整场录音的曲目、时间点与输出设置。');
  }
  if(!existing){
    data.projects.push(updated);
    addLog(updated,'创建项目','已记录','从整场录音分轨工作区创建。');
  }
  if(updated.recordingParts?.length&&previousParts!==JSON.stringify(updated.recordingParts)){
    addLog(updated,'录音拼接','成功',updated.recordingParts.join(' → ')+' → '+updated.sources[0]);
  }
  persist();
  return existing||updated;
}
function saveSplitResult(id,{job,duration,settings,format,error,note}){
  const p=pBy(id);
  if(!p)return;
  const target=job.source+' → '+job.target;
  if(error)addLog(p,'错误记录','失败',target+'\n'+error);
  if(!error){
    const details=target+'\n'+duration.toFixed(3)+' s · '+format+' · '+settings.rate/1000+' kHz · '+settings.gain+' dB · 声场 '+settings.width+'% · 混响 '+settings.reverb+'%'+(note?'\n'+note:'');
    const steps=['split','rename','tags','format','check'];
    if(settings.cover)steps.push('cover');
    if(settings.gain!==0)steps.push('gain');
    if(settings.width!==100)steps.push('width');
    if(settings.reverb!==0)steps.push('reverb');
    if(effectsEnabled(settings.effects))steps.push('effects');
    for(const step of steps)addLog(p,stepNames[step],'成功',details+(step==='effects'?'\n'+JSON.stringify(settings.effects):''));
  }
  persist();
}
function finishSplitProject(id,{success,failed,stopped,cover}){
  const p=pBy(id);
  if(!p)return;
  if(success&&cover)p.coverStatus='已嵌入';
  if(success&&!failed)p.status=stopped?'已分轨':'已完成';
  if(failed)p.status='处理失败';
  if(stopped)addLog(p,'分轨','已记录','处理中止，已完成 '+success+' 轨。');
  persist();
}
function download(name,content,mime='application/json'){
  const link=document.createElement('a');link.href=URL.createObjectURL(new Blob([content],{type:mime}));link.download=name;link.click();setTimeout(()=>URL.revokeObjectURL(link.href),5000);
}
function page(){const hash=location.hash.slice(1);return labels[hash]?hash:'projects';}
function go(next){if(page()===next)render();else location.hash=next;}
function openProject(id){projectId=id;projectTab='overview';go('projects');main.focus();}
function render(){
  const current=page();
  $('#crumb').textContent=labels[current];
  document.querySelectorAll('[data-nav]').forEach(a=>{a.classList.toggle('active',a.dataset.nav===current);if(a.dataset.nav===current)a.setAttribute('aria-current','page');else a.removeAttribute('aria-current');});
  if(!data)return;
  if(current==='splitter'){
    splitter ||= createSplitter({getRule:()=>data.naming,getProjects:()=>data.projects,saveProject:saveSplitProject,saveResult:saveSplitResult,finishProject:finishSplitProject,openProject,openLocalScript,existing:splitExisting,newSession:()=>{splitExisting=null;splitter=null;render();}});
    if(!main.contains(splitter))main.replaceChildren(splitter);
    return;
  }
  main.innerHTML=({projects:()=>projectId&&pBy(projectId)?projectView(pBy(projectId)):dashboard(),scripts:scriptsView,history:()=>historyView(),templates:templatesView})[current]();
  if(current==='scripts') updatePlan();
}
const normalize=s=>String(s ?? '').normalize('NFD').replace(/\p{Diacritic}/gu,'').toLowerCase();
const grouped=(key,p)=>!key||key==='all'||(key==='complete'&&p.status==='已完成')||(key==='repair'&&['处理失败','需修复'].includes(p.status))||(key==='open'&&!['已完成','处理失败','需修复'].includes(p.status));
function visibleProjects(){
  const query=normalize(filters.q.trim());
  const visible=data.projects.filter(p=>grouped(activeGroup,p)&&Object.keys(fields).every(key=>{
    if(!filters[key])return true;
    if(key==='composer')return [...(p.composer||'').split(' / '),...p.tracks.flatMap(t=>(t.tags?.Composer||'').split(' / '))].includes(filters[key]);
    return p[key]===filters[key]||(['performer','conductor'].includes(key)&&p.tracks.some(t=>t.tags?.[key==='performer'?'Soloist':'Conductor']===filters[key]));
  })&&(!query||normalize([p.name,...Object.keys(fields).map(key=>p[key]),...p.sources,...p.tracks.flatMap(t=>[t.filename,...Object.values(t.tags||{})])].join(' ')).includes(query)));
  const sort=currentSort();
  if(sort==='manual')return visible;
  return visible.sort((a,b)=>sort==='name'?a.name.localeCompare(b.name):sort==='oldest'?(a.date||'9999').localeCompare(b.date||'9999'):(b.date||'').localeCompare(a.date||''));
}
function listRows(){
  const visible=visibleProjects();
  $('#project-results').innerHTML=projectTable(visible);
}
function projectTable(rows){
  const manual=currentSort()==='manual';
  const move=(label,action,project,disabled)=>'<button type="button" class="button quiet" data-action="'+action+'" data-id="'+esc(project.id)+
    '" aria-label="'+esc(label+' '+project.name)+'"'+(disabled?' disabled':'')+'>'+label+'</button>';
  return (rows.length?table(['录音项目','演出日期','城市 / 音乐厅','录音来源','曲目','状态','操作'],rows.map((p,index)=>tr([
    '<div class="project-title"><span class="tile" aria-hidden="true">'+esc((p.performer||p.conductor||p.name).slice(0,1))+'</span><div><strong>'+esc(p.name)+'</strong><small>'+esc([p.performer,p.conductor].filter(Boolean).join(' · ')||p.composer||'')+'</small></div></div>',
    esc(p.date||'—'),esc([p.city,p.venue].filter(Boolean).join(' · ')||'—'),esc(p.origin||'—'),String(p.tracks.length),badge(p.status),
    '<div class="actions project-actions">'+button('打开 ↗','open','quiet',p.id)+
    (manual?move('上移','move-project-up',p,index===0)+move('下移','move-project-down',p,index===rows.length-1):'')+
    '<button type="button" class="button danger" data-action="delete-project" data-id="'+esc(p.id)+'" aria-label="'+esc('删除 '+p.name)+'">删除</button></div>'
  ])).join('')):'<div class="empty">没有符合条件的录音项目。</div>')+'<div class="table-foot">显示 '+rows.length+' / '+data.projects.length+' 个项目'+
    (manual?' · 上移和下移在当前筛选结果内生效。':'')+'</div>';
}
function moveProject(project,direction){
  if(!project||currentSort()!=='manual')return;
  const visible=visibleProjects();
  const index=visible.indexOf(project);
  const neighbor=visible[index+(direction==='move-project-up'?-1:1)];
  if(index<0||!neighbor)return;
  const from=data.projects.indexOf(project),to=data.projects.indexOf(neighbor);
  [data.projects[from],data.projects[to]]=[data.projects[to],data.projects[from]];
  persist();
  listRows();
  const rowId='[data-id="'+CSS.escape(project.id)+'"]';
  const rowActions=$('#project-results');
  (rowActions.querySelector('[data-action="'+direction+'"]'+rowId+':enabled')||rowActions.querySelector('[data-action="open"]'+rowId))?.focus();
  toast('已移动「'+project.name+'」。');
}
function deleteProject(project){
  if(!project)return;
  const related=()=>splitter?.projectIdentifier()===project.id;
  if(related()&&(splitter.isProcessing()||splitter.waitForMediaSave())){toast('请先停止或等待此项目的音频处理。');return;}
  modal('删除录音项目','<p>确定从工作台删除「'+esc(project.name)+'」？原始录音、已导出的文件和处理记录仍会保留。</p>'+
    (related()&&splitter.hasUnsavedChanges()?'<p>这个项目当前的分轨编辑也将被清除。</p>':''),()=>{
    if(related()){
      if(splitter.isProcessing()||splitter.waitForMediaSave())throw Error('请先停止或等待此项目的音频处理。');
      splitter.dispose();
      splitter=null;
      splitExisting=null;
    }
    if(projectId===project.id)projectId='';
    if(historyProject===project.id)historyProject='';
    if(scriptProject===project.id)resetScript(null);
    modify(()=>{
      data.projects=data.projects.filter(item=>item.id!==project.id);
      changeLog(project,'从工作台删除录音项目。');
    });
    toast('已删除「'+project.name+'」。');
  },'确认删除');
}
function dashboard(){
  const stats=[['all','▤','全部项目'],['open','◷','整理中'],['complete','✓','已完成'],['repair','◇','待修复']];
  const f=key=>{
    const values=key==='status'?statuses:unique(data.projects.flatMap(p=>{
      if(key==='composer')return [...(p.composer||'').split(' / '),...p.tracks.flatMap(t=>(t.tags?.Composer||'').split(' / '))];
      if(['performer','conductor'].includes(key))return [p[key],...p.tracks.map(t=>t.tags?.[key==='performer'?'Soloist':'Conductor'])];
      return [p[key]];
    }));
    return field(fields[key],select('filter-'+key,[['','全部'],...values],filters[key]));
  };
  return '<section class="hero"><div><h1>录音项目</h1><p>归档演出录音，管理分轨、专辑标签、封面和处理进度。</p></div>'+'<div class="actions">'+button('打开项目文件','open-project-file')+'</div></section><section class="stats" aria-label="项目状态概览">'+stats.map(([key,icon,label])=>'<button class="stat '+(activeGroup===key?'selected':'')+'" data-action="group" data-id="'+key+'"><i aria-hidden="true">'+icon+'</i><span><strong>'+data.projects.filter(p=>grouped(key,p)).length+'</strong><small>'+label+'</small></span></button>').join('')+'</section>'+
  '<section class="card pad" aria-label="筛选"><div class="filters-top">'+field('搜索项目',input('filter-q',filters.q,'type="search" placeholder="项目、人物、曲目或文件名"'))+f('status')+f('origin')+f('date')+'</div>'+(moreFilters?'<div class="filterbar">'+['performer','conductor','composer','city','venue'].map(f).join('')+'</div>':'')+'<div class="filters-extra">'+button(moreFilters?'收起筛选 ▴':'更多筛选 ▾','filters','quiet')+button('清除筛选','reset-filters','quiet')+'</div></section>'+
  '<section class="card"><div class="card-head pad" style="margin:0"><h2>项目列表</h2><div class="actions"><label class="count">排序 '+select('sort',projectSorts,currentSort())+'</label>'+button('＋ 新建项目','new','primary')+'</div></div><div id="project-results">'+projectTable(visibleProjects())+'</div></section>';
}
function projectView(p){
  const tabLabels={overview:'项目概览',tracks:'分轨与标签',history:'项目历史'};
  const tabs='<nav class="tabs" aria-label="项目详情">'+Object.entries(tabLabels).map(([key,label])=>'<button class="tab '+(projectTab===key?'active':'')+'" data-action="project-tab" data-id="'+key+'">'+label+'</button>').join('')+'</nav>';
  return '<div class="section-gap">'+button('← 返回项目列表','back','quiet')+'</div>'+head(p.name,[p.date,p.city,p.venue].filter(Boolean).join(' · '),button('保存为本地文件','export-project','',p.id)+button('编辑项目','edit','',p.id)+button('在软件中分轨 ↗','project-splitter','',p.id)+button('生成脚本 ↗','project-script','primary',p.id))+tabs+
    (projectTab==='tracks'?tracksView(p):projectTab==='history'?historyView(p):overview(p));
}
function overview(p){
  const details=Object.entries(fields).map(([key,label])=>'<div><dt>'+label+'</dt><dd>'+(key==='status'?badge(p[key]):esc(p[key]||'—'))+'</dd></div>').join('');
  const s=p.settings||defaults;
  return '<section class="card pad"><div class="card-head"><h2>演出与录音</h2></div><dl class="detail-list">'+details+'<div><dt>乐团</dt><dd>'+esc(p.orchestra||'—')+'</dd></div></dl>'+(p.notes?'<p class="hint detail-note">'+esc(p.notes)+'</p>':'')+'</section>'+
    '<section class="card pad"><div class="card-head"><h2>原始文件</h2>'+button('管理文件','sources','',p.id)+'</div>'+(p.sources.length?'<div class="source-list">'+p.sources.map(s=>'<code>'+esc(s)+'</code>').join('')+'</div>':'<p class="hint">尚未登记 WAV、FLAC、MP3 或 MKV 文件。</p>')+
    (p.recordingParts?.length?'<p class="hint">原始片段拼接顺序</p><ol>'+p.recordingParts.map(s=>'<li>'+esc(s)+'</li>').join('')+'</ol>':'')+'</section>'+
    '<section class="card pad"><div class="card-head"><h2>封面与音频设置</h2>'+button('编辑设置','settings','',p.id)+'</div><dl class="detail-list">'+[['封面文件',p.coverFile||'自动查找 cover.png / cover.jpg'],['封面状态',p.coverStatus],['封底文件',p.backCoverFile||'未设置'],['专辑标签日期 / 年份',p.albumDate||p.date||'—'],['输出格式',s.cover&&s.format==='WAV'?'FLAC（嵌入封面）':s.format],['采样率',s.rate/1000+' kHz'],['增益',s.gain+' dB'],['声场宽度',s.width+'%'],['混响',s.reverb+'%'],['输出目录',p.output]].map(([key,v])=>'<div><dt>'+key+'</dt><dd>'+esc(v)+'</dd></div>').join('')+'</dl></section>';
}
function tracksView(p){
  const rows=p.tracks.map((t,i)=>{
    const filename=filenameFor(p.namingRule||data.naming||naming,t,{...t.tags,Track:t.tags?.Track||String(i+1).padStart(2,'0')},i);
    return '<tr class="track-row"><td>'+(i+1)+'</td><td><strong>'+esc(t.tags?.Title||'未命名曲目')+'</strong><div class="subtle">'+esc(t.tags?.Composer||'')+'</div></td><td>'+esc(t.source||'—')+'</td><td class="mono">'+esc(t.start||'00:00')+' – '+esc(t.end||'结束')+'</td><td>'+esc(filename||'—')+'</td><td><div class="actions">'+button('编辑','track-edit','',t.id)+(i>0?button('↑','track-up','quiet',t.id):'')+(i<p.tracks.length-1?button('↓','track-down','quiet',t.id):'')+'</div></td></tr>';
  }).join('');
  return '<section class="card"><div class="pad card-head" style="margin:0"><h2>曲目顺序与标签</h2>'+button('＋ 添加曲目','track-add','primary')+'</div>'+(rows?table(['顺序','Title / Composer','原始文件','分轨时间','英文文件名（不含扩展名）',''],rows):'<div class="empty">添加曲目后可录入分轨点、英文文件名和标签。</div>')+'</section>';
}
function historyView(p){
  const queryProject=p?.id||historyProject;
  const rows=data.logs.filter(l=>(!queryProject||l.projectId===queryProject)&&(!historyOperation||l.operation===historyOperation)&&(!historyOutcome||l.result===historyOutcome));
  const actions=button('导入 FFmpeg 日志','import-log')+button('＋ 添加记录','log-add','primary',p?.id||'');
  const filtersMarkup='<div class="form-grid">'+(!p?field('录音项目',select('history-project',[['','所有项目'],...data.projects.map(x=>[x.id,x.name])],queryProject),'span2'):'')+field('操作',select('history-operation',[['','所有操作'],...operations],historyOperation))+field('结果',select('history-outcome',[['','所有结果'],...outcomes],historyOutcome))+'</div>';
  return (p?'<div class="card-head"><h2>项目历史</h2><div class="actions">'+actions+'</div></div>':head('处理记录','按操作与结果查看项目历史、错误和修订。',actions))+'<section class="card pad">'+filtersMarkup+'</section><section class="card">'+(rows.length?table(['时间','项目','操作','结果','详情'],rows.map(l=>tr([esc(dateTime(l.time)),esc(pBy(l.projectId)?.name||l.projectName||'—'),esc(l.operation),badge(l.result),'<div class="log-note">'+esc(l.message)+'</div>'])).join('')):'<div class="empty">暂无符合条件的处理记录。</div>')+'<div class="table-foot">'+rows.length+' 条记录</div></section>';
}
function templatesView(){
  return head('模板与命名','保存常用处理参数和英文文件命名规则。',button('＋ 新建模板','template-add','primary'))+
    '<section class="card pad"><div class="card-head"><h2>默认曲目命名规则</h2>'+button('编辑规则','naming')+'</div><code>'+esc(data.naming)+'</code><p class="hint">可用变量 '+esc(['Source','Order',...tags].map(key=>'{'+key+'}').join(' · '))+'</p></section>'+
    '<div class="template-grid">'+data.templates.map(t=>'<article class="card pad"><div class="card-head"><h2>'+esc(t.name)+'</h2></div><p class="subtle">'+esc(t.settings.format+' · '+t.settings.rate/1000+' kHz · '+t.settings.gain+' dB · '+t.settings.width+'% 声场 · '+t.settings.reverb+'% 混响')+'</p><p><code>'+esc(t.rule)+'</code></p><div class="actions">'+button('编辑','template-edit','',t.id)+button('用于脚本 ↗','template-use','quiet',t.id)+'</div></article>').join('')+'</div>';
}
function settingsFields(s){
  return '<div class="form-grid settings-grid">'+field('输出格式',select('format',['FLAC','WAV','MP3','MKV'],s.format))+field('采样率',select('rate',[[44100,'44.1 kHz'],[48000,'48 kHz'],[96000,'96 kHz']],s.rate))+'<div class="gain-controls">'+field('增益（dB）',input('gain',s.gain,'type="number" min="-60" max="30" step="0.1" required'))+limiterForm(s.effects)+'</div>'+field('声场宽度（%）',input('width',s.width,'type="number" min="0" max="250" step="1" required'))+field('混响（%）',input('reverb',s.reverb,'type="number" min="0" max="100" step="1" required'))+'</div>'+effectsForm(s.effects)+'<div class="check-row section-gap">'+[['split','按曲目分轨'],['metadata','重写标签'],['cover','嵌入封面']].map(([key,label])=>'<label><input type="checkbox" name="'+key+'"'+(s[key]?' checked':'')+'> '+label+'</label>').join('')+'</div>';
}
function readSettings(form,validate=true){
  const f=getForm(form);
  return {format:f.format,rate:Number(f.rate),gain:Number(f.gain),width:Number(f.width),reverb:Number(f.reverb),cover:!!f.cover,split:!!f.split,metadata:!!f.metadata,effects:readEffects(form,validate)};
}
function resetScript(p,template){
  scriptProject=p?.id||'';
  scriptState={settings:{...defaults,...(template?.settings||p?.settings)},rule:template?.rule||(p?.settings?.split===false?'{Source}':p?.namingRule||data.naming),output:template?.output||(p?.autoOutput||p?.output==='Processed'?sessionFolderName(p):p?.output)||'Processed',template:template?.id||'',selected:null};
  generated=null;
}
function buildProjectScript(project,plan){
  generated=renderScript(plan);
  addLog(project,'脚本生成','已记录',plan.jobs.length+' 项 · '+plan.format+' · '+plan.settings.rate/1000+' kHz · '+plan.settings.gain+' dB · 声场 '+plan.settings.width+'% · 混响 '+plan.settings.reverb+'% · 输出 '+plan.output);
  persist();
}
function openLocalScript(project,plan){
  resetScript(project);
  buildProjectScript(project,plan);
  go('scripts');
}
function scriptsView(){
  const p=pBy(scriptProject)||data.projects[0];
  if(!p)return head('脚本生成')+'<section class="card empty">请先创建录音项目。 '+button('新建项目','new','primary')+'</section>';
  if(!scriptState||p.id!==scriptProject)resetScript(p);
  const st=scriptState;
  const rows=st.settings.split?p.tracks.map((t,i)=>[t.id,(i+1)+' · '+(t.tags?.Title||'未命名曲目')]):p.sources.map(s=>[s,s]);
  return head('脚本生成','将所选音频和封面放在同一个文件夹中，在该文件夹打开 Windows PowerShell，粘贴脚本运行。')+
    '<div class="script-grid"><section class="card pad"><form id="script-form"><div class="form-grid">'+field('录音项目',select('script-project',data.projects.map(x=>[x.id,x.name]),p.id),'wide')+field('处理模板',select('script-template',[['','项目设置 / 自定义'],...data.templates.map(t=>[t.id,t.name])],st.template),'wide')+field('输出文件夹',input('output',st.output,'required maxlength="100"'),'span2')+field('命名规则',input('rule',st.rule,'required maxlength="240"'),'span2')+'</div><div class="section-gap">'+settingsFields(st.settings)+'</div><p class="hint">默认输出 48 kHz。WAV 选中嵌入封面时转为 FLAC。WAV 的可写标签有限，完整专辑标签请选 FLAC。</p><details class="section-gap"><summary>选择处理项目（'+rows.filter(([id])=>!st.selected||st.selected.includes(id)).length+' / '+rows.length+'）</summary><div class="source-list">'+(rows.map(([id,label])=>'<label><input type="checkbox" name="job" value="'+val(id)+'"'+(!st.selected||st.selected.includes(id)?' checked':'')+'> '+esc(label)+'</label>').join('')||'<span class="hint">尚未登记项目文件或分轨。</span>')+'</div></details><div id="plan-errors" class="alert" role="status"></div><div class="actions section-gap"><button class="button primary" type="submit" id="make-script">生成 PowerShell 脚本</button>'+button('编辑项目 ↗','open','quiet',p.id)+'</div></form></section>'+
    '<section class="card pad"><div class="card-head"><h2>PowerShell 预览</h2><div class="actions">'+button('复制','copy-script')+button('下载 .ps1','download-script')+'</div></div><textarea id="script-text" class="script-preview" aria-label="PowerShell 脚本" spellcheck="false" readonly placeholder="生成的脚本将在这里显示。">'+esc(generated||'')+'</textarea><p class="hint">脚本需要 PATH 中的 ffmpeg 和 ffprobe。输出目录内的 processing-*.jsonl 可导入处理记录。</p></section></div>';
}
function readScript(form){
  const f=getForm(form),prev=scriptState;
  scriptState={...prev,settings:readSettings(form,false),output:f.output.trim(),rule:f.rule.trim(),selected:new FormData(form).getAll('job')};
}
function updatePlan(){
  const form=$('#script-form');
  if(!form)return null;
  const p=pBy(scriptProject),st=scriptState;
  const plan=makePlan(p,st.settings,st.rule,st.output,st.selected);
  if(!st.rule)plan.errors.unshift('请填写输出文件命名规则。');
  $('#plan-errors').innerHTML=plan.errors.map(esc).join('<br>');
  $('#make-script').disabled=!!plan.errors.length;
  main.querySelectorAll('[data-action="copy-script"],[data-action="download-script"]').forEach(b=>b.disabled=!generated);
  return plan;
}
function modal(title,body,submit,submitLabel='保存'){
  dialog.innerHTML='<form><div class="dialog-head"><h2 id="modal-title">'+title+'</h2>'+button('关闭','close','quiet')+'</div><div class="dialog-body">'+body+'</div><div class="form-error" role="alert"></div><div class="dialog-foot">'+button('取消','close','quiet')+'<button class="button primary" type="submit">'+submitLabel+'</button></div></form>';
  dialog.setAttribute('aria-labelledby','modal-title');
  dialog.showModal();
  const form=dialog.querySelector('form');
  form.addEventListener('submit',event=>{
    event.preventDefault();
    try{submit(form,getForm(form));dialog.close();}
    catch(error){dialog.querySelector('.form-error').textContent=error.message;}
  });
}
function editProject(p){
  const original=p?{...p}:blankProject();
  modal(p?'编辑项目':'新建录音项目','<div class="form-grid">'+field('项目名称',input('name',original.name,'required maxlength="180"'),'wide')+Object.entries(fields).map(([key,label])=>field(label,key==='status'?select(key,statuses,original[key]):input(key,original[key],key==='date'?'type="date"':'maxlength="220"'),'span2')).join('')+field('乐团',input('orchestra',original.orchestra,'maxlength="160"'),'span2')+field('备注',area('notes',original.notes,4,'maxlength="4000"'),'wide')+(!p?field('原始文件名，每行一个',area('sources','',3,'placeholder="例如 Recording 1.wav"'),'wide'):'')+'</div>',(_,f)=>{
    const next={...original,...Object.fromEntries(['name',...Object.keys(fields),'orchestra','notes'].map(key=>[key,f[key].trim()]))};
    if(!next.name)throw Error('请填写项目名称。');
    if(!p)next.sources=parseSources(f.sources);
    modify(()=>{
      if(p){Object.assign(p,next);changeLog(p,'更新项目资料。'+(original.status===f.status?'':'状态：'+original.status+' → '+f.status));}
      if(!p){data.projects.push(next);addLog(next,'创建项目','已记录','新建录音项目。');}
      projectId=next.id;projectTab='overview';
    });
    go('projects');
  },p?'保存项目':'创建项目');
}
function parseSources(value){
  const result=value.split(/\r?\n/).map(x=>x.trim()).filter(Boolean);
  if(result.some(x=>!sourceValid(x)))throw Error('每行填写一个当前文件夹中的 WAV、FLAC、MP3 或 MKV 文件名。');
  if(new Set(result.map(x=>x.toLowerCase())).size!==result.length)throw Error('原始文件名有重复，请检查列表。');
  return result;
}
function editSources(p){
  modal('管理原始文件',field('文件名，每行一个',area('sources',p.sources.join('\n'),8,'placeholder="Recording 1.wav"'))+'<p class="hint">支持 WAV、FLAC、MP3 和 MKV。请填写文件名及扩展名。</p>',(_,f)=>{
    const next=parseSources(f.sources);
    const missing=p.tracks.filter(t=>t.source&&!next.some(x=>x.toLowerCase()===t.source.toLowerCase()));
    if(missing.length)throw Error('有 '+missing.length+' 条曲目使用已移除的文件，请先修改对应曲目的原始文件。');
    modify(()=>{p.sources=next;p.tracks.forEach(t=>{if(t.source)t.source=next.find(x=>x.toLowerCase()===t.source.toLowerCase());});changeLog(p,'更新原始文件列表，共 '+next.length+' 个文件。');});
  });
}
function projectSettings(p){
  modal('编辑封面与音频设置','<div class="form-grid">'+field('封面文件',input('coverFile',p.coverFile,'placeholder="cover.png / cover.jpg"'),'span2')+field('封面状态',select('coverStatus',coverStates,p.coverStatus),'span2')+field('输出文件夹',input('output',p.output,'required maxlength="100"'),'wide')+'</div><div class="section-gap">'+settingsFields(p.settings)+'</div>',(form,f)=>{
    const coverFile=f.coverFile.trim(),output=f.output.trim();
    if(coverFile&&!coverValid(coverFile))throw Error('封面须为 PNG 或 JPG 文件名。');
    if(!outputValid(output))throw Error('请填写有效的 Windows 输出文件夹名。');
    modify(()=>{Object.assign(p,{coverFile,autoOutput:output===p.output&&p.autoOutput,output,coverStatus:f.coverStatus,settings:readSettings(form)});changeLog(p,'更新封面与音频设置。');});
  });
}
function editTrack(p,previous){
  const t=previous||blankTrack();
  modal(previous?'编辑曲目':'添加曲目','<div class="form-grid">'+field('原始文件',select('source',[['','待填写'],...p.sources],t.source),'span2')+field('英文文件名（可留空按规则生成）',input('filename',t.filename,'maxlength="200"'),'span2')+field('起点',input('start',t.start,'placeholder="00:00"'),'span2')+field('终点（留空至文件结束）',input('end',t.end,'placeholder="MM:SS 或 HH:MM:SS"'),'span2')+tags.map(key=>field(key,input('tag-'+key,t.tags?.[key]||'','maxlength="280"'+(key==='Title'?' required':'')),key==='Title'?'wide':'span2')).join('')+'</div>'+(previous?'<div class="section-gap">'+button('删除此曲目','track-delete','danger',t.id)+'</div>':''),(_,f)=>{
    const start=f.start.trim(),end=f.end.trim(),a=timeSeconds(start)??0,b=timeSeconds(end);
    if(!Number.isFinite(a)||(b!==null&&(!Number.isFinite(b)||b<=a)))throw Error('请核对时间格式和起止顺序。');
    const values=Object.fromEntries(tags.map(key=>[key,f['tag-'+key].trim()]));
    if(!values.Title)throw Error('请填写 Title。');
    const next={...t,source:f.source,filename:f.filename.trim(),start,end,tags:values};
    modify(()=>{if(previous)Object.assign(previous,next);if(!previous)p.tracks.push(next);changeLog(p,(previous?'更新曲目：':'添加曲目：')+values.Title);});
  });
}
function editTemplate(t){
  const initial=t||{id:crypto.randomUUID(),name:'',rule:data.naming,output:'Processed',settings:defaults};
  modal(t?'编辑模板':'新建模板','<div class="form-grid">'+field('模板名称',input('name',initial.name,'required maxlength="120"'),'wide')+field('命名规则',input('rule',initial.rule,'required maxlength="240"'),'span2')+field('输出文件夹',input('output',initial.output,'required maxlength="100"'),'span2')+'</div><div class="section-gap">'+settingsFields(initial.settings)+'</div>'+(t?'<div class="section-gap">'+button('删除模板','template-delete','danger',t.id)+'</div>':''),(form,f)=>{
    const name=f.name.trim(),rule=f.rule.trim(),output=f.output.trim();
    if(!name||!rule||!outputValid(output))throw Error('请填写模板名称、命名规则和有效输出目录。');
    modify(()=>{const next={...initial,name,rule,output,settings:readSettings(form)};if(t)Object.assign(t,next);if(!t)data.templates.push(next);addLog(null,'修订记录','已记录',(t?'更新模板：':'创建模板：')+name);});
  });
}
function editLog(p){
  if(!data.projects.length){toast('请先创建项目。');return;}
  modal('添加处理记录','<div class="form-grid">'+field('项目',select('project',data.projects.map(x=>[x.id,x.name]),p?.id||historyProject||data.projects[0].id),'wide')+field('操作',select('operation',operations,historyOperation||'质量检查'),'span2')+field('结果',select('result',outcomes,'成功'),'span2')+field('更新项目状态',select('status',[['','保持当前状态'],...statuses],''),'wide')+field('文件、参数或结果说明',area('message','',5,'required maxlength="4000"'),'wide')+'</div>',(_,f)=>{
    if(!f.message.trim())throw Error('请填写处理结果。');
    modify(()=>{const project=pBy(f.project);if(f.status&&project.status!==f.status){changeLog(project,'状态：'+project.status+' → '+f.status);project.status=f.status;}addLog(project,f.operation,f.result,f.message.trim());});
  });
}
function openFile(mode){
  file.dataset.mode=mode;
  file.accept=mode==='log'?'.jsonl':'.json';
  file.value='';
  file.click();
}
async function exportProject(project){
  const name=project.name.replace(/[<>:"/\\|?*\u0000-\u001f]/gu,'-').trim().slice(0,110)||'recording-project';
  const filename='CR Studio - '+name+'.crstudio.json';
  const content=JSON.stringify({
    format:'classical-recording-studio/project',version:1,
    project:{...project,namingRule:project.namingRule||data.naming},
    logs:data.logs.filter(log=>log.projectId===project.id)
  });
  if(window.studioNative?.saveProjectFile){
    const {value,error}=await window.studioNative.saveProjectFile(filename,content);
    if(error)throw Error(error);
    if(!value)return;
  }else download(filename,content);
  toast('已导出项目文件。');
}
function openProjectDocument(raw){
  let document;
  try{document=JSON.parse(raw);}
  catch{throw Error('无法读取项目文件，请选择工作台导出的 .crstudio.json 文件。');}
  if(document?.format!=='classical-recording-studio/project'||document.version!==1)throw Error('项目文件格式或版本不受支持。请选择 .crstudio.json 文件。');
  const {project,logs}=document;
  const validId=id=>typeof id==='string'&&/^[\w-]{1,160}$/u.test(id);
  if(!validBackup({naming:project?.namingRule,projects:[project],logs,templates:[]})||
    !validId(project.id)||!project.name.trim()||project.tracks.some(track=>!validId(track.id))||
    logs.some(log=>!validId(log.id)||log.projectId!==project.id))throw Error('项目文件的内容不完整或不正确。');
  const previous=pBy(project.id);
  const projectLogs=data.logs.filter(log=>log.projectId===project.id);
  if(previous&&JSON.stringify({...previous,namingRule:previous.namingRule||data.naming})===JSON.stringify(project)&&JSON.stringify(projectLogs)===JSON.stringify(logs)){
    openProject(previous.id);
    toast('已打开现有项目。');
    return;
  }
  function importProject(){
    const imported=previous?{...project,id:crypto.randomUUID()}:project;
    const used=new Set(data.logs.map(log=>log.id));
    const incomingLogs=logs.map(log=>({...log,id:used.has(log.id)?crypto.randomUUID():log.id,projectId:imported.id}));
    const updated={...data,projects:[...data.projects,imported],logs:[...incomingLogs,...data.logs]};
    if(new TextEncoder().encode(JSON.stringify(updated)).length>1800000)throw Error('工作区空间不足。请先导出备份并整理现有项目。');
    data=updated;
    persist();
    openProject(imported.id);
    toast(previous?'已作为副本打开项目文件。':'已打开项目文件。');
  }
  if(previous){
    modal('打开项目副本','<p>工作区已有不同版本的「'+esc(previous.name)+'」。将作为另一个项目打开，保留当前版本。</p>',importProject,'作为副本打开');
    return;
  }
  importProject();
}
const isString=x=>typeof x==='string';
const optionalString=x=>x==null||isString(x);
function validSettings(s){return !!s&&effectsValid(s.effects)&&['WAV','FLAC','MP3','MKV'].includes(s.format)&&[44100,48000,96000].includes(s.rate)&&['gain','width','reverb'].every(k=>Number.isFinite(s[k]))&&['split','cover','metadata'].every(k=>typeof s[k]==='boolean');}
function validLocalMedia(media){
  if(media===undefined)return true;
  const id=value=>typeof value==='string'&&/^[\w-]{1,80}$/u.test(value);
  const audio=value=>!!value&&id(value.id)&&sourceValid(value.name);
  return !!media&&Array.isArray(media.audio)&&media.audio.length<=500&&media.audio.every(audio)&&
    (media.merged==null||audio(media.merged))&&[media.cover,media.backCover].every(value=>value==null||value===''||id(value));
}
function validBackup(x){
  const ids=items=>new Set(items.map(v=>v?.id)).size===items.length;
  return x&&isString(x.naming)&&Array.isArray(x.projects)&&ids(x.projects)&&x.projects.every(p=>p&&isString(p.id)&&isString(p.name)&&statuses.includes(p.status)&&Object.keys(fields).every(k=>optionalString(p[k]))&&['output','notes','orchestra','coverFile','coverStatus','backCoverFile','albumDate'].every(k=>optionalString(p[k]))&&validSettings(p.settings)&&validLocalMedia(p.localMedia)&&(p.recordingParts===undefined||Array.isArray(p.recordingParts)&&p.recordingParts.every(x=>isString(x)&&sourceValid(x)))&&Array.isArray(p.sources)&&p.sources.every(x=>isString(x)&&sourceValid(x))&&Array.isArray(p.tracks)&&ids(p.tracks)&&p.tracks.every(t=>t&&isString(t.id)&&['source','start','end','filename'].every(k=>optionalString(t[k]))&&t.tags&&tags.every(k=>optionalString(t.tags[k]))))&&Array.isArray(x.templates)&&ids(x.templates)&&x.templates.every(t=>t&&['id','name','rule','output'].every(k=>isString(t[k]))&&validSettings(t.settings))&&Array.isArray(x.logs)&&ids(x.logs)&&x.logs.every(l=>l&&['id','projectId','projectName','time','operation','result','message'].every(k=>isString(l[k])));
}
async function importFile(){
  const selected=file.files[0];
  if(!selected)return;
  const mode=file.dataset.mode;
  try{
    if(selected.size>1800000)throw Error('文件过大，请拆分后导入。');
    const raw=await selected.text();
    if(mode==='project'){openProjectDocument(raw);return;}
    if(mode==='backup'){
      const parsed=JSON.parse(raw), incoming=parsed.data??parsed;
      if(!validBackup(incoming))throw Error('备份格式无效，请选择由工作台导出的 JSON。');
      modal('导入工作区备份','<p>此备份包含 '+incoming.projects.length+' 个项目、'+incoming.templates.length+' 个模板和 '+incoming.logs.length+' 条记录。导入会替换当前工作区。</p>',()=>{
        projectId='';scriptState=null;historyProject='';modify(()=>{data=incoming;});toast('已导入备份');
      },'替换工作区');
      return;
    }
    const records=raw.split(/\r?\n/).filter(s=>s.trim()).map(s=>JSON.parse(s));
    if(!records.length||records.some(r=>r?.schema!==1||!isString(r.runId)||!isString(r.projectId)||!isString(r.source)||!isString(r.output)||!isString(r.message)||!Number.isFinite(Date.parse(r.time))||!Array.isArray(r.steps)||!['success','failed','skipped'].includes(r.status)))throw Error('请选择脚本生成的 processing-*.jsonl 文件。');
    const p=pBy(records[0].projectId);
    if(!p||records.some(r=>r.projectId!==p.id))throw Error('日志中的项目与当前工作区不匹配。');
    const keys=new Set(data.logs.map(l=>l.importKey).filter(Boolean));
    const pending=records.filter(r=>{const key=r.runId+'|'+r.output;if(keys.has(key))return false;keys.add(key);return true;});
    if(!pending.length){toast('这些日志已导入。');return;}
    modal('导入 FFmpeg 处理日志','<p>'+esc(p.name)+' · '+pending.length+' 条文件处理结果</p>'+field('更新项目状态',select('status',[['','保持当前状态'],...statuses],pending.some(r=>r.status==='failed')?'处理失败':'')),(_,f)=>{
      modify(()=>{
        if(f.status&&p.status!==f.status){changeLog(p,'状态：'+p.status+' → '+f.status);p.status=f.status;}
        if(pending.some(r=>r.status==='success'&&r.steps.includes('cover')))p.coverStatus='已嵌入';
        for(const r of pending){
          const params=r.settings??{};
          const detail=r.source+' → '+r.output+(r.message?'\n'+r.message:'')+(Number.isFinite(r.duration)?'\n时长 '+r.duration.toFixed(3)+' s':'')+(params.rate?'\n'+params.rate/1000+' kHz · '+params.gain+' dB · 声场 '+params.width+'% · 混响 '+params.reverb+'%':'');
          const actions=r.status==='success'?[...new Set(r.steps.filter(x=>Object.hasOwn(stepNames,x)))]:[];
          if(!actions.length)addLog(p,r.status==='failed'?'错误记录':'质量检查',r.status==='failed'?'失败':'跳过',detail,{time:r.time,importKey:r.runId+'|'+r.output});
          actions.forEach((step,i)=>addLog(p,stepNames[step],'成功',detail+(step==='effects'&&params.effects?'\n'+JSON.stringify(params.effects):''),{time:r.time,...(i===0?{importKey:r.runId+'|'+r.output}:{})}));
        }
      });
      toast('已导入 '+pending.length+' 条文件结果');
    },'导入日志');
  }catch(error){toast(error.message);}
}
document.addEventListener('click',async event=>{
  const target=event.target.closest('[data-action],[data-nav],.brand');
  if(!target)return;
  if(target.matches('[data-nav],.brand')){
    if(target.dataset.nav==='projects'||target.matches('.brand')){projectId='';if(page()==='projects')render();}
    return;
  }
  const {action,id}=target.dataset;
  const p=pBy(projectId),item=pBy(id),t=p?.tracks.find(x=>x.id===id);
  switch(action){
    case 'open':openProject(id);break;
    case 'open-project-file':if(data)openFile('project');break;
    case 'export-project':
      if(item)try{await exportProject(item);}catch(error){toast(error.message);}
      break;
    case 'new':editProject();break;
    case 'move-project-up':
    case 'move-project-down':moveProject(item,action);break;
    case 'delete-project':deleteProject(item);break;
    case 'edit':editProject(item);break;
    case 'back':projectId='';render();break;
    case 'project-tab':projectTab=id;render();break;
    case 'group':activeGroup=activeGroup===id?'':id;render();break;
    case 'filters':moreFilters=!moreFilters;render();break;
    case 'reset-filters':Object.keys(filters).forEach(k=>filters[k]='');activeGroup='';render();break;
    case 'sources':editSources(item);break;
    case 'settings':projectSettings(item);break;
    case 'track-add':editTrack(p);break;
    case 'track-edit':editTrack(p,t);break;
    case 'track-up':
    case 'track-down':{
      const i=p.tracks.indexOf(t),j=i+(action==='track-up'?-1:1);
      if(j<0||j>=p.tracks.length)break;
      modify(()=>{[p.tracks[i],p.tracks[j]]=[p.tracks[j],p.tracks[i]];changeLog(p,'移动曲目 '+t.tags.Title+' 至第 '+(j+1)+' 位。');});
      break;
    }
    case 'track-delete':{
      dialog.close();
      modal('删除曲目','<p>删除 '+esc(t.tags.Title)+' 的分轨与标签记录。</p>',()=>modify(()=>{p.tracks=p.tracks.filter(x=>x!==t);changeLog(p,'删除曲目：'+t.tags.Title);}), '删除');
      break;
    }
    case 'project-splitter':
      if(splitter?.isProcessing()||splitter?.waitForMediaSave()){toast('分轨正在运行，请先停止或等待完成。');break;}
      splitter?.dispose();splitExisting=item;splitter=null;go('splitter');break;
    case 'project-script':resetScript(item);go('scripts');break;
    case 'template-add':editTemplate();break;
    case 'template-edit':editTemplate(data.templates.find(x=>x.id===id));break;
    case 'template-use':resetScript(pBy(scriptProject)||data.projects[0],data.templates.find(x=>x.id===id));go('scripts');break;
    case 'template-delete':{
      dialog.close();
      const template=data.templates.find(x=>x.id===id);
      modal('删除模板','<p>删除 '+esc(template.name)+'。</p>',()=>modify(()=>{data.templates=data.templates.filter(x=>x.id!==id);addLog(null,'修订记录','已记录','删除模板：'+template.name);}), '删除');
      break;
    }
    case 'naming':modal('编辑命名规则',field('默认英文曲目命名规则',input('rule',data.naming,'required maxlength="240"')),(_,f)=>{if(!f.rule.trim())throw Error('请填写命名规则。');modify(()=>{data.naming=f.rule.trim();addLog(null,'修订记录','已记录','更新默认命名规则：'+data.naming);});});break;
    case 'log-add':editLog(item);break;
    case 'import-log':openFile('log');break;
    case 'copy-script':{
      try{await navigator.clipboard.writeText(generated);toast('已复制 PowerShell 脚本');}catch{$('#script-text').select();toast('已选中脚本，可按 Ctrl+C 复制');}
      break;
    }
    case 'download-script':download('classical-recording-workflow.ps1','\ufeff'+generated,'text/plain;charset=utf-8');break;
    case 'close':dialog.close();break;
  }
});
main.addEventListener('input',event=>{
  const {name,value}=event.target;
  if(name==='filter-q'){filters.q=value;listRows();}
  if(event.target.form?.id==='script-form'&&(['rule','output','gain','width','reverb'].includes(name)||name?.startsWith('effect-'))){
    readScript(event.target.form);generated=null;$('#script-text').value='';updatePlan();
  }
});
main.addEventListener('change',event=>{
  const {name,value}=event.target;
  if(name?.startsWith('filter-')&&name!=='filter-q'){filters[name.slice(7)]=value;listRows();}
  if(name==='sort'&&projectSorts.some(([key])=>key===value)){data.projectSort=value;persist();listRows();}
  if(name==='history-project'){historyProject=value;render();}
  if(name==='history-operation'){historyOperation=value;render();}
  if(name==='history-outcome'){historyOutcome=value;render();}
  if(event.target.form?.id!=='script-form')return;
  const p=pBy(scriptProject);
  if(name==='script-project'){resetScript(pBy(value));render();return;}
  if(name==='script-template'){resetScript(p,data.templates.find(t=>t.id===value));render();return;}
  const previous=scriptState.settings.split;
  readScript(event.target.form);
  generated=null;
  if(name==='split'){
    scriptState.selected=null;
    if(previous&&scriptState.rule===(p.namingRule||data.naming))scriptState.rule='{Source}';
    if(!previous&&scriptState.rule==='{Source}')scriptState.rule=p.namingRule||data.naming;
    render();return;
  }
  if(name==='job'){const details=event.target.closest('details');details.querySelector('summary').textContent='选择处理项目（'+scriptState.selected.length+' / '+details.querySelectorAll('[name="job"]').length+'）';}
  $('#script-text').value='';updatePlan();
});
main.addEventListener('submit',event=>{
  if(event.target.id!=='script-form')return;
  event.preventDefault();readScript(event.target);
  const result=updatePlan();
  if(result.errors.length)return;
  const {plan}=result;
  buildProjectScript(pBy(scriptProject),plan);
  $('#script-text').value=generated;updatePlan();toast('PowerShell 脚本已生成');
});
window.addEventListener('hashchange',()=>{render();main.focus();});
window.addEventListener('online',save);
window.addEventListener('beforeunload',()=>{if(dirty&&!discardOnExit)localRecovery();});
$('#backup').addEventListener('click',()=>{if(data)download('classical-recording-studio.json',JSON.stringify({version:1,data},null,2));});
$('#restore').addEventListener('click',()=>{if(data)openFile('backup');});
file.addEventListener('change',importFile);
async function initialize(){
  let local,workspaceLoaded=false;
  try{local=JSON.parse(localStorage.getItem('classical-studio-recovery')||'null');}catch{}
  try{
    const response=await fetch('/api/workspace',{cache:'no-store'});
    const remote=await response.json();
    if(!response.ok)throw Error(remote.error);
    workspaceLoaded=true;
    revision=remote.revision;
    data=remote.data||seed();
    sync('本机工作区 · 已连接');
    if(validBackup(local?.data)){
      if(local.revision===revision){data=local.data;persist();}
      if(local.revision!==revision){download('classical-recording-recovery.json',JSON.stringify({version:1,data:local.data},null,2));toast('工作区有较新版本。本机草稿已另存为备份。');try{localStorage.removeItem('classical-studio-recovery');}catch{}}
    }
  }catch(error){
    data=validBackup(local?.data)?local.data:seed();
    revision=local?.revision||0;
    sync('未连接 · 本机草稿');
    toast(error.message||'工作区连接失败');
  }
  render();
  if(workspaceLoaded&&page()==='splitter'&&splitter&&window.studioNative?.startupCheck){
    dirty=true;
    await save();
    if(dirty)return;
    window.studioNative.startupReady();
  }
}
async function confirmExit(id){
  await splitter?.waitForMediaSave()?.catch(()=>{});
  if(saving)await saving;
  const native=window.studioNative;
  const draft=splitter;
  if(!draft?.hasUnsavedChanges()&&!draft?.isProcessing()&&!dirty){
    native.answerClose(id,true);
    return;
  }
  const active=!!draft?.isProcessing();
  const prompt=document.createElement('dialog');
  prompt.setAttribute('aria-label','退出前保存项目');
  prompt.innerHTML='<div class="dialog-head"><h2>退出前保存项目</h2></div>'+
    '<div class="dialog-body"><p>'+ (active?'音频正在处理。退出会停止处理，已完成的文件会保留。':'当前有未保存的项目更改。')+
    '</p><p class="form-error" role="alert"></p><p class="hint" role="status" data-exit-status></p></div>'+
    '<div class="dialog-foot"><button class="button quiet" type="button" data-exit="cancel" autofocus>取消</button>'+
    '<button class="button" type="button" data-exit="discard">不保存并退出</button>'+
    '<button class="button primary" type="button" data-exit="save">保存并退出</button></div>';
  let answered=false,waiting=false;
  const buttons=[...prompt.querySelectorAll('button')];
  const error=prompt.querySelector('[role="alert"]');
  const status=prompt.querySelector('[data-exit-status]');
  function answer(allow){
    if(answered)return;
    answered=true;
    native.answerClose(id,allow);
    prompt.close();
  }
  prompt.addEventListener('cancel',event=>{
    event.preventDefault();
    if(!waiting)answer(false);
  });
  prompt.addEventListener('close',()=>{if(!answered)native.answerClose(id,false);prompt.remove();},{once:true});
  for(const button of buttons)button.onclick=async()=>{
    const action=button.dataset.exit;
    if(action==='cancel'){answer(false);return;}
    waiting=true;
    for(const item of buttons)item.disabled=true;
    error.textContent='';
    try{
      if(draft?.isProcessing()){
        status.textContent='正在停止音频处理…';
        await draft.stopForExit();
      }
      if(action==='save'){
        status.textContent='正在保存项目…';
        if(draft?.hasUnsavedChanges())await draft.saveForExit();
        await save();
        if(dirty)throw Error(lastSaveError||'未能写入工作区。');
      }
      if(action==='discard'){
        discardOnExit=true;
        try{localStorage.removeItem('classical-studio-recovery');}catch{}
      }
      answer(true);
    }catch(failure){
      error.textContent=(failure.message||'保存失败。')+' 软件仍保持打开。';
      status.textContent='';
      waiting=false;
      for(const item of buttons)item.disabled=false;
    }
  };
  document.body.append(prompt);
  prompt.showModal();
}
window.studioNative?.onCloseRequest?.(id=>{
  void confirmExit(id).catch(error=>{
    toast(error.message||'未能准备退出。');
    window.studioNative.answerClose(id,false);
  });
});

void initialize();

if(window.studioNative?.llmGet){
  for(const [selector,action] of [['#llm-settings',openLlmSettings],['#llm-files',()=>openLlmFiles()]]){
    const button=$(selector);button.hidden=false;button.onclick=()=>void action();
  }
}
