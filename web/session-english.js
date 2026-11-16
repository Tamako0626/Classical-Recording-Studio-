import {composerCredit} from './program-credits.js';
import {composerFromCredit} from './work-catalog.js';

const han=/\p{Script=Han}/u;
const clean=value=>String(value??'').normalize('NFKC').trim().replace(/\s+/gu,' ');
const lookupKey=value=>clean(value).normalize('NFD').replace(/[\p{Diacritic}\p{Punctuation}\s]/gu,'').toLowerCase();

function aliases(entries){
  return new Map(entries.flatMap(([english,...names])=>[english,...names].map(name=>[lookupKey(name),english])));
}
const people=aliases([
  ['Michel Dalberto','米歇尔·达尔贝托','米歇尔·达尔贝多','米歇尔·达尔伯托','达尔贝托','达尔贝多'],
  ['Dénes Várjon','德内什·瓦尔扬','德内什·瓦尔约恩','德奈什·瓦尔扬','德内什·瓦尔荣','瓦尔扬','瓦尔荣'],
  ['Dmitri Alexeev','德米特里·阿列克谢耶夫','德米特里·阿列克谢夫','德米特里 Alexeev'],
  ['Hans Zender','汉斯·岑德尔','汉斯·森德尔','汉斯·曾德尔','汉斯·泽德'],
  ['Franz Welser-Möst','弗朗茨·韦尔泽-莫斯特','弗朗茨·威尔瑟-莫斯特','韦尔泽-莫斯特'],
  ['Michael Sanderling','米夏埃尔·桑德林','迈克尔·桑德林'],
  ['Ingo Metzmacher','英戈·梅茨马赫'],
  ['Seiji Ozawa','小泽征尔','小澤征爾'],
  ['Yehudi Menuhin','耶胡迪·梅纽因','梅纽因'],
  ['Ernest Ansermet','欧内斯特·安塞美','安塞美'],
  ['Mikhail Voskresensky','米哈伊尔·沃斯克列先斯基','沃斯克列先斯基']
]);
const cities=aliases([
  ['Shanghai','上海','上海市'],['Harbin','哈尔滨','哈爾濱','哈尔滨市'],['Beijing','北京','北京市'],['Hamburg','汉堡','漢堡'],
  ['Vienna','维也纳','維也納'],['Montreux','蒙特勒','蒙特鲁'],['Lucerne','琉森','卢塞恩','盧塞恩'],
  ['Zurich','苏黎世','蘇黎世','Zürich'],['Munich','慕尼黑'],['Lausanne','洛桑'],
  ['Paris','巴黎'],['London','伦敦','倫敦'],['Berlin','柏林'],['Tokyo','东京','東京'],
  ['New York','纽约','紐約','纽约市'],['Boston','波士顿','波士頓'],
  ['Guangzhou','广州','廣州','广州市'],['Shenzhen','深圳','深圳市'],['Hong Kong','香港']
]);
const venues=[
  {city:'Shanghai',map:aliases([['Shanghai Symphony Hall','上海交响音乐厅','上海交响乐团音乐厅','上交音乐厅'],['Shanghai Concert Hall','上海音乐厅'],['Cadillac Shanghai Concert Hall','凯迪拉克上海音乐厅','Cadillac · Shanghai Concert Hall'],['Shanghai Grand Theatre','上海大剧院']])},
  {city:'Beijing',map:aliases([['National Centre for the Performing Arts','国家大剧院','中國國家大劇院','中国国家大剧院','NCPA']])},
  {city:'Vienna',map:aliases([['Musikverein','维也纳音乐协会','Vienna Musikverein'],['Golden Hall, Musikverein','维也纳金色大厅','维也纳音乐协会金色大厅']])},
  {city:'Lucerne',map:aliases([['KKL Luzern','琉森文化会议中心','卢塞恩文化会议中心','KKL Lucerne']])},
  {city:'Hamburg',map:aliases([['Elbphilharmonie Hamburg','易北爱乐音乐厅','Elbphilharmonie']])},
  {city:'New York',map:aliases([['Carnegie Hall','卡内基音乐厅']])}
];
const orchestras=aliases([
  ['Shanghai Symphony Orchestra','上海交响乐团','上海交響樂團','上交'],
  ['Hamburger Symphoniker','汉堡交响乐团','德国汉堡交响乐团','Hamburg Symphony Orchestra'],
  ['Cleveland Orchestra','克利夫兰管弦乐团','克利夫兰交响乐团','The Cleveland Orchestra'],
  ['Vienna Philharmonic','维也纳爱乐乐团','Wiener Philharmoniker'],
  ['Berliner Philharmoniker','柏林爱乐乐团','柏林愛樂樂團','Berlin Philharmonic'],
  ['Lucerne Symphony Orchestra','琉森交响乐团','卢塞恩交响乐团','Luzerner Sinfonieorchester'],
  ['Boston Symphony Orchestra','波士顿交响乐团','波士頓交響樂團','BSO']
]);
const origins=aliases([
  ['Personal Live Recording','现场自录','现场录音（自录）','本人现场录制','观众自录'],
  ['Live Recording','现场录音','现场','現場錄音'],['Historical Live Recording','历史现场录音'],
  ['Radio Broadcast','广播','广播录音','电台广播','电台录音','广播实况','電台錄音'],
  ['Television Broadcast','电视广播','电视录音'],['Commercial Release','商业发行','唱片发行'],
  ['Private Recording','私人录音'],['Archival Recording','档案录音','档案馆录音'],
  ['Studio Recording','录音室录音'],['Online Stream','网络直播','网络回放','网络录音']
]);
const eventTypes=aliases([
  ['Piano Recital','钢琴独奏音乐会','钢琴独奏会','钢琴独奏专场','钢琴音乐会'],
  ['Solo Recital','独奏音乐会','独奏会'],['Violin Recital','小提琴独奏会','小提琴独奏音乐会'],
  ['Piano Masterclass','钢琴大师课','钢琴大师班'],['Masterclass','大师课','大师班'],
  ['Symphony Concert','交响音乐会','交响乐音乐会'],['Chamber Music Concert','室内乐音乐会'],
  ['Concert','音乐会'],['Live Recording','现场录音']
]);
const titleTerms=[
  ['钢琴独奏音乐会','Piano Recital'],['钢琴独奏会','Piano Recital'],['钢琴独奏专场','Piano Recital'],
  ['钢琴大师课','Piano Masterclass'],['小提琴独奏会','Violin Recital'],['独奏音乐会','Solo Recital'],
  ['独奏会','Solo Recital'],['大师课','Masterclass'],['交响音乐会','Symphony Concert'],
  ['室内乐音乐会','Chamber Music Concert'],['音乐会','Concert']
];
const composerName=value=>{
  const credit=composerCredit(value);
  return credit&&!credit.title?credit.composer:composerFromCredit(value);
};
const locationFor=value=>venues.find(venue=>venue.map.has(lookupKey(value)));
const normalizeList=(value,find)=>{
  const exact=find(value);
  if(exact)return exact;
  const resolved=value.split(/\s*[、；;\/，,]\s*/u).map(find);
  return resolved.every(Boolean)?[...new Set(resolved)].join(' / '):'';
};

function sessionTitle(value,context){
  const direct=eventTypes.get(lookupKey(value));
  if(direct)return direct;
  let title=value, subject='';
  for(const [key,map] of [['performer',people],['conductor',people],['orchestra',orchestras]]){
    const raw=context[key];
    const resolved=map.get(lookupKey(raw))||(!han.test(raw)?raw:'');
    if(raw&&resolved&&title.startsWith(raw)&&title!==raw){
      subject=resolved;
      title=title.slice(raw.length).replace(/^[\s·:：–—-]+/u,'');
      break;
    }
  }
  // A full event title is translated only if every part is recognized.
  const suffix=titleTerms.find(([name])=>title.endsWith(name));
  if(!suffix)return '';
  const pre=title.slice(0,-suffix[0].length).trim();
  if(!pre&&subject)return subject+' – '+suffix[1];
  if(!pre)return suffix[1];
  const named=people.get(lookupKey(pre))||orchestras.get(lookupKey(pre));
  if(named)return [subject,named,suffix[1]].filter(Boolean).join(' – ');
  const composer=composerName(pre.replace(/的$/u,'').trim());
  if(composer)return (subject?subject+' – ':'')+composer.split(' ').at(-1)+' '+suffix[1];
  if(!han.test(pre)&&pre.length<=170&&/^[\p{L}\p{N} .·'’&–-]+$/u.test(pre)){
    return [subject,pre,suffix[1]].filter(Boolean).join(' – ');
  }
  return '';
}
export function normalizeSession(fields){
  const original=Object.fromEntries(Object.entries(fields).map(([key,value])=>[key,clean(value)]));
  const value=key=>original[key]||'';
  const changes={}, review={}, verified=[];
  const hall=locationFor(value('venue'));
  const city=cities.get(lookupKey(value('city')));
  const mismatch=hall&&value('city')&&lookupKey(city||value('city'))!==lookupKey(hall.city);
  for(const [key,input] of Object.entries(original)){
    if(!input)continue;
    let resolved='';
    if(key==='name')resolved=sessionTitle(input,original);
    if(key==='city')resolved=city;
    if(key==='venue'&&!mismatch)resolved=hall?.map.get(lookupKey(input));
    if(key==='orchestra')resolved=normalizeList(input,name=>orchestras.get(lookupKey(name)));
    if(key==='origin')resolved=origins.get(lookupKey(input));
    if(key==='performer'||key==='conductor')resolved=normalizeList(input,name=>people.get(lookupKey(name)));
    if(key==='composer')resolved=normalizeList(input,composerName);
    if(key==='date'){
      const date=new Date(input+'T00:00:00Z');
      if(/^\d{4}-\d{2}-\d{2}$/u.test(input)&&!Number.isNaN(+date)&&date.toISOString().slice(0,10)===input)verified.push(key);
      else review[key]='请核对日期，格式为 YYYY-MM-DD。';
      continue;
    }
    if(mismatch&&(key==='city'||key==='venue')){
      review[key]='场馆目录对应 '+hall.city+'，请核对城市与音乐厅。';
      continue;
    }
    if(resolved&&resolved!==input){changes[key]=resolved;continue;}
    if(resolved&&!han.test(input)){verified.push(key);continue;}
    if(han.test(input))review[key]='未找到可靠的英文对应，请核对。';
    if(!resolved&&!han.test(input)&&['performer','conductor','composer','orchestra','venue','city'].includes(key))review[key]='目录暂无此名称，请核对拼写。';
  }
  return {changes,review,verified};
}

export function modelSessionReview(raw,original){
  let result;
  try{result=JSON.parse(raw.trim().replace(/^```(?:json)?\s*|\s*```$/giu,''));}
  catch{throw Error('模型未返回有效的校对格式，请重试。');}
  if(!result?.fields||typeof result.fields!=='object'||Array.isArray(result.fields))throw Error('模型未返回有效的校对格式，请重试。');
  const suggestions=[];
  for(const [key,before] of Object.entries(original)){
    if(!before||key==='date')continue;
    const item=result.fields[key];
    if(!item||typeof item!=='object')continue;
    const value=typeof item.value==='string'?item.value.trim():'';
    const note=typeof item.note==='string'?item.note.trim().slice(0,280):'';
    const latin=!!value.match(/\p{L}/u)&&(value.match(/\p{L}/gu)||[]).every(letter=>/\p{Script=Latin}/u.test(letter));
    const after=value&&value.length<=280&&!/[\x00-\x1f\x7f]/u.test(value)&&latin?value:'';
    if(after||note)suggestions.push({key,before,after,note});
  }
  const proposed={...original,...Object.fromEntries(suggestions.filter(item=>item.after).map(item=>[item.key,item.after]))};
  const {review}=normalizeSession(proposed);
  const originalReview=normalizeSession(original).review;
  for(const item of suggestions){
    if(!['venue','city'].includes(item.key))continue;
    const conflict=[originalReview[item.key],review[item.key]].find(note=>note?.startsWith('场馆目录对应 '));
    if(conflict){item.note=[item.note,conflict].filter(Boolean).join(' ');item.needsReview=true;}
  }
  return suggestions;
}

const folderText=value=>clean(value).replace(/[<>:"/\\|?*\u0000-\u001f]+/gu,' - ').replace(/\s+/gu,' ').trim().replace(/[. ]+$/u,'');
function removeFolderPrefix(text,prefix){
  if(!prefix||!text.toLowerCase().startsWith(prefix.toLowerCase()))return text;
  const rest=text.slice(prefix.length);
  return !rest||/^[\s·:：–—-]/u.test(rest)?rest.replace(/^[\s·:：–—-]+/u,''):text;
}
export function sessionFolderName(fields){
  const {changes,verified}=normalizeSession(fields);
  const date=verified.includes('date')?clean(fields.date):'';
  const performer=folderText(changes.performer||fields.performer||changes.conductor||fields.conductor||changes.orchestra||fields.orchestra);
  const city=folderText(changes.city||fields.city);
  const inputOrigin=clean(changes.origin||fields.origin);
  const origin=/^(?:Personal|Historical) Live Recording$/iu.test(inputOrigin)?'Live Recording':folderText(inputOrigin);
  let title=folderText(changes.name||fields.name);
  title=removeFolderPrefix(title,date);
  for(const name of [performer,folderText(fields.performer||fields.conductor||fields.orchestra)])title=removeFolderPrefix(title,name);
  for(const name of [city,folderText(fields.city)])title=removeFolderPrefix(title,name);
  title=title.replace(/\s*\(([^()]*)\)\s*$/u,(suffix,inner)=>{
    const known=origins.get(lookupKey(inner));
    return known&&origin&&(lookupKey(known)===lookupKey(origin)||known.endsWith('Live Recording')&&origin==='Live Recording')?'':suffix;
  });
  const leader=[date,performer].filter(Boolean).join(' ');
  const description=[city,title].filter(Boolean).join(' ');
  let stem=[leader,description].filter(Boolean).join(' - ')||'Processed';
  if(/^(?:con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])(?:\.|$)/iu.test(stem))stem='Album - '+stem;
  const suffix=origin?' ('+origin.slice(0,43).trimEnd()+')':'';
  const available=100-suffix.length;
  let short=stem.slice(0,available).replace(/[\uD800-\uDBFF]$/u,'').replace(/[. –—-]+$/u,'');
  if(!short)short='Processed';
  return short+suffix;
}
