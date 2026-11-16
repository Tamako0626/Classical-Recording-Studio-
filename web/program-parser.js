import {englishProgramTrack} from './program-english.js';
import {composerCredit,opusLine,verifiedProgramWork} from './program-credits.js';
import {timeSeconds} from './script.js';
import {isBilingualWorkTitle,mergeBilingualTracks} from './program-bilingual.js';

const labels = {
  name:'场次(?:名称)?|专辑(?:名)?|音乐会(?:名称)?|活动名称|标题|concert|event|album|session|title',
  date:'演出日期|演出时间|日期|时间|performance date|date|when',
  performer:'钢琴(?:家)?|小提琴|演奏家|演奏者|独奏(?:家)?|独唱|pianist|piano|violin|soloist|performer',
  conductor:'指挥(?:家)?|conductor',
  composer:'作曲(?:家)?|composer',
  orchestra:'乐团|管弦乐团|orchestra|ensemble',
  city:'城市|city',
  venue:'场馆|音乐厅|演出地点|地点|venue|location|hall',
  origin:'录音来源|recording source|source'
};
const fieldPattern=Object.entries(labels).map(([key,pattern])=>[key,new RegExp('^(?:'+pattern+')\\s*[:：]\\s*(.+)$','iu')]);
const sessionLabels=Object.values(labels).join('|');
const inlineSessionField=new RegExp('[ \\t]+(?=(?:'+sessionLabels+')\\s*[:：])','giu');
const separatedSessionField=new RegExp('^(?:'+sessionLabels+')(?=\\s+[|｜–—-]\\s+|\\s{2,})','iu');
const datePattern=/(?<!\d)(\d{4})\s*(?:[./-]|年)\s*(\d{1,2})\s*(?:[./-]|月)\s*(\d{1,2})(?:\s*日)?(?!\d)/u;
const englishDatePattern=/\b(?:(\d{1,2})\s+(Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:t(?:ember)?)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\s+(\d{4})|(Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:t(?:ember)?)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\s+(\d{1,2}),?\s+(\d{4}))\b/i;
const programHeader=/^(?:演出)?(?:曲目(?:单)?|节目单|program(?:me)?|repertoire)\s*[:：]?\s*$/iu;
const heading=/^(?:上半场|下半场|中场休息|返场(?:曲目)?|中场|[上下]半場|intermission|interval|encores?|(?:first|second) half)\s*[:：]?\s*$/iu;
const trackPrefix=/^(?:[•●▪*]\s*|[-–—]\s+|(?:\d{1,3}|[一二三四五六七八九十]+)[.、)）]\s*|\d{1,3}\s+(?!\d))/u;
const movement=/^(?:[IVX]{1,5}\s*[.)、]|第[一二三四五六七八九十\d]+乐章|(?:movement|mvt\.?)\s+[\dIVX]+)/iu;
const clock='\\d{1,3}[:：][0-5]\\d(?:[:：][0-5]\\d)?(?:[.,]\\d{1,3})?';
const range=new RegExp('^\\s*('+clock+')\\s*(?:[-–—~～]|至)\\s*('+clock+'|结束|end)(?=\\s|$)','iu');
const startOnly=new RegExp('^\\s*('+clock+')\\s+(?:[-–—|]\\s*)?','u');
const trailingTimes=new RegExp('\\s+[（(]?('+clock+')(?:\\s*(?:[-–—~～]|至)\\s*('+clock+'|结束|end))?[)）]?\\s*$','iu');
const workLike=/协奏曲|奏鸣曲|交响曲|夜曲|练习曲|组曲|前奏曲|幻想曲|舞曲|变奏曲|四重奏|进行曲|间奏曲|叙事曲|谐谑曲|嬉游曲|小品|月光|(?:[贝柴马布][一二三四五六七八九])|肖协|concerto|symphon(?:y|ie)|sonat[ae]|nocturne|étude|prelude|suite|quartet|overture|variations|fantas[yi]|clair de lune|\b(?:op|bwv|kv?|hob|wab|d|l)\.?\s*\d/iu;
const nonWork=/^(?:(?:page|p\.)\s*\d+(?:\s*(?:of|\/)\s*\d+)?|第\s*\d+\s*页|(?:第\s*)?\d+\s*[\/／]\s*\d+|(?:scan(?: me| to .*)?|qr code|ticket(?:s|ing)?|booking|subscribe|follow us|all rights reserved|organis[ez]ed by|presented by|sponsored by|photo(?:graphy)? by|duration|doors open|contact(?: us)?)(?:\b|[:：])|(?:扫码|扫描二维码|关注公众号|关注我们|票价|购票|订票|售票|温馨提示|演出须知|入场须知|主办|承办|协办|赞助|票务|版权所有|摄影|演出时长|曲目(?:及|和|与)顺序|演出曲目以|返回顶部))|(?:https?:\/\/|www\.|[\p{L}\d._+-]+@[\p{L}\d.-]+\.[a-z]{2,})/iu;
const pageTime=new RegExp('^'+clock+'(?:\\s*(?:[-–—~～]|至)\\s*'+clock+')?$','u');
const recognizedTitle=/\b(?:allegro|adagio|andante|andantino|largo|lento|presto|vivace|menuetto|scherzo|rondo|barcarolle|mazurka|waltz|ballade|capriccio|impromptu|intermezzo|fantas(?:y|ia|ie)|gavotte|sarabande|polonaise|berceuse|lied|song|requiem|fugue|encore|estampes|images|kinderszenen|arabesque|stück(?:e)?|moments musicaux|gaspard de la nuit|la campanella|oiseaux tristes)\b|无词歌|摇篮曲|回旋曲|阿拉贝斯克|安魂曲|赋格|即兴曲|夜之幽灵|船歌|快板|慢板|行板|小步舞曲/iu;

export function isProgramJunk(value){
  const title=value.normalize('NFKC').trim();
  if(!/\p{L}/u.test(title)||pageTime.test(title)||nonWork.test(title))return true;
  if(/@/u.test(title)&&!workLike.test(title)&&!recognizedTitle.test(title))return true;
  return /^(?:[A-Z]{1,3}\s*){1,2}$/u.test(title)&&!movement.test(title);
}

const venueLike=/音乐厅|音乐堂|大剧院|演艺中心|concert hall|musikverein|concertgebouw|carnegie hall|philharmonie|wigmore hall|tonhalle/iu;
const recTitle=/音乐会|独奏会|大师课|专场|recital|masterclass|concert\b/iu;
const composerNames=/^(?:巴赫|贝多芬|莫扎特|肖邦|舒伯特|舒曼|李斯特|德彪西|拉威尔|柴可夫斯基|布鲁克纳|马勒|勃拉姆斯|拉赫玛尼诺夫|海顿|门德尔松|斯克里亚宾|普罗科菲耶夫|拉赫曼尼诺夫|巴托克|斯特拉文斯基|瓦格纳|Bach|Beethoven|Mozart|Chopin|Schubert|Schumann|Liszt|Debussy|Ravel|Tchaikovsky|Bruckner|Mahler|Brahms|Rachmanin(?:off|ov)|Haydn|Mendelssohn|Scriabin|Prokofiev|Bart[oó]k|Stravinsky|Wagner)$/iu;
const normalizeClock=value=>(value||'').replaceAll('：',':').replace(',','.');
function dateIn(text){
  const numeric=text.match(datePattern);
  const english=text.match(englishDatePattern);
  if(!numeric&&!english)return '';
  const months=['jan','feb','mar','apr','may','jun','jul','aug','sep','oct','nov','dec'];
  const [y,m,d]=numeric?numeric.slice(1,4).map(Number):[Number(english[3]||english[6]),months.indexOf((english[2]||english[4]).slice(0,3).toLowerCase())+1,Number(english[1]||english[5])];
  const parsed=new Date(Date.UTC(y,m-1,d));
  return parsed.getUTCFullYear()===y&&parsed.getUTCMonth()===m-1&&parsed.getUTCDate()===d
    ?String(y).padStart(4,'0')+'-'+String(m).padStart(2,'0')+'-'+String(d).padStart(2,'0'):'';
}
function cleanLine(value){
  return value.normalize('NFC').replace(/[\u200B\uFEFF]/gu,'').replace(/([\p{Script=Han}]) +(?=\p{Script=Han})/gu,'$1').replace(/[ \t]+/gu,' ').trim();
}

const canonicalOpus=title=>title.match(/(?:\bop\.?|作品)\s*(\d+)/iu)?.[1]||'';
function canMergeCues(first,second){
  if(['start','end'].some(key=>first[key]&&second[key]&&timeSeconds(first[key])!==timeSeconds(second[key])))return false;
  const start=timeSeconds(first.start||second.start);
  const end=timeSeconds(first.end||second.end);
  return start===null||end===null||end>start;
}

export function parseSessionText(text){
  const lines=text.split(/\r?\n/u).map(line=>{
    const stripped=line.trim().replace(/^[•●▪]\s*/u,'');
    return stripped
      .replace(separatedSessionField,label=>label+':')
      .replace(/:\s*[|｜–—-]\s+/u,': ')
      .replace(inlineSessionField,'\n')
      .split(/[|｜;；]/u)
      .map(part=>part.trim())
      .filter(Boolean)
      .join('\n');
  });
  return parseProgram(lines.join('\n')).session;
}

export function parseProgram(text){
  const session={};
  const tracks=[];
  const unassigned=[];
  const review=[];
  const excluded=[];
  let inProgram=false;
  let groupComposer='';
  let previousWork=null;
  let corrected=0;
  const verified=new Map();
  const explicitCredits=new Map();
  let parentWork='';
  for(let line of text.split(/\r?\n/u).map(cleanLine).filter(Boolean)){
    if(programHeader.test(line)){inProgram=true;groupComposer='';parentWork='';previousWork=null;continue;}
    if(heading.test(line)){inProgram=true;parentWork='';groupComposer='';previousWork=null;continue;}
    const label=fieldPattern.find(([,pattern])=>pattern.test(line));
    if(label){
      const [key,pattern]=label;
      const value=line.match(pattern)[1].trim();
      if(key==='composer'&&inProgram){groupComposer=value;parentWork='';continue;}
      const field=key==='date'?dateIn(value):value.slice(0,280);
      if(field)session[key]=field;
      else unassigned.push(line);
      continue;
    }
    const date=dateIn(line);
    if(date&&!workLike.test(line)){
      session.date??=date;
      const remainder=line.replace(datePattern,'').replace(englishDatePattern,'').replace(/^\s*(?:[0-2]?\d:[0-5]\d\s*)?[|｜·–—-]?\s*/u,'').trim();
      if(venueLike.test(remainder))session.venue??=remainder.slice(0,280);
      continue;
    }
    const role=line.match(/^(.*?)\s*[,，|／/（(]\s*(piano|violin|conductor|钢琴|小提琴|指挥|独奏)\s*[)）]?$/iu);
    if(role){session[/conductor|指挥/iu.test(role[2])?'conductor':'performer']??=role[1];continue;}
    const raw=line;
    if(nonWork.test(line)){excluded.push(raw);continue;}
    if(opusLine.test(line)){
      const checked=verified.get(previousWork);
      if(previousWork&&!canonicalOpus(previousWork.title)&&(!checked||canonicalOpus(checked.title)===canonicalOpus(line))){
        previousWork.title=(previousWork.title.replace(/[,，]\s*$/u,'')+', '+line).slice(0,280);
      }else excluded.push(raw);
      continue;
    }
    previousWork=null;
    const listed=trackPrefix.test(line);
    if(listed)line=line.replace(trackPrefix,'');
    const labeledComposer=listed?line.match(/\s+[|｜]\s*(?:Composer|作曲家)\s*[:：]\s*(.{1,140})$/iu):null;
    if(labeledComposer)line=line.slice(0,labeledComposer.index).trim();
    if(heading.test(line)){inProgram=true;parentWork='';groupComposer='';previousWork=null;continue;}
    const rangeMatch=line.match(range);
    const timestamp=rangeMatch||line.match(startOnly);
    if(timestamp)line=line.slice(timestamp[0].length).replace(/^\s*(?:[-—–|]+)\s*/u,'');
    if(isProgramJunk(line)){excluded.push(raw);continue;}
    if(!workLike.test(line)&&!listed){
      if(venueLike.test(line)&&!recTitle.test(line)){session.venue??=line;continue;}
      if(recTitle.test(line)){session.name??=line;continue;}
    }
    let explicitComposer=labeledComposer&&!/^(?:unknown|未知)$/iu.test(labeledComposer[1].trim())?labeledComposer[1].trim():'';
    const credit=composerCredit(line);
    if(credit){
      groupComposer=credit.composer;
      explicitComposer=credit.composer;
      line=credit.title;
      parentWork='';
      if(!line)continue;
    }
    const composerLine=line.match(/^([^:：]{2,60})\s*[:：]\s*(.*)$/u);
    if(composerLine&&composerNames.test(composerLine[1].trim())){
      groupComposer=composerLine[1].trim();
      explicitComposer=groupComposer;
      line=composerLine[2].trim();
      parentWork='';
      if(!line)continue;
    }else if(composerNames.test(line)){
      groupComposer=line;parentWork='';continue;
    }
    const atEnd=timestamp?null:line.match(trailingTimes);
    if(atEnd)line=line.slice(0,atEnd.index).trim();
    // A damaged prefix before a clear Chinese work title is not a composer credit.
    if(/^[^:：]{1,8}[:：]\s*\p{Script=Han}/u.test(line))line=line.replace(/^[^:：]+[:：]\s*/u,'');
    if(isProgramJunk(line)){excluded.push(raw);continue;}
    const checked=verifiedProgramWork(line,explicitComposer);
    const isMovement=movement.test(line);
    const strong=!!checked||workLike.test(line)||recognizedTitle.test(line)||isBilingualWorkTitle(line)||isMovement;
    const end=rangeMatch?.[2]||atEnd?.[2]||'';
    const track={title:line.slice(0,280),composer:(checked?.composer||explicitComposer||(labeledComposer?'':groupComposer)).slice(0,280),start:normalizeClock(timestamp?.[1]||atEnd?.[1]),end:/^end$|^结束$/iu.test(end)?'':normalizeClock(end)};
    if(!strong&&!listed&&!englishProgramTrack(track).englishOriginal){
      if(inProgram||timestamp||atEnd)review.push(raw);
      else unassigned.push(raw);
      continue;
    }

    if(isMovement&&parentWork)line=parentWork+' – '+line;
    if(!isMovement&&workLike.test(line))parentWork=line;
    const hasMovements=isMovement&&tracks.at(-1)?.title===parentWork&&!tracks.at(-1)?.start;
    if(hasMovements)tracks.pop();
    if(checked&&groupComposer!==checked.composer){
      if(groupComposer)corrected++;
      groupComposer=checked.composer;
    }
    const entry={...track,title:line.slice(0,280)};
    tracks.push(entry);
    previousWork=entry;
    if(explicitComposer)explicitCredits.set(entry,explicitComposer);
    if(checked)verified.set(entry,checked);
  }
  const ordered=[];
  const matching=new Map();
  let catalogueMerges=0;
  const confirmedWorks=new Set(verified.values());
  for(const track of tracks){
    const checked=verified.get(track)||verifiedProgramWork(track.title,explicitCredits.get(track)||'',confirmedWorks);
    if(!checked){ordered.push(track);continue;}
    const opus=canonicalOpus(track.title);
    if(track.composer!==checked.composer||opus&&opus!==canonicalOpus(checked.title))corrected++;
    const result={...track,title:checked.title,composer:checked.composer};
    if(/\p{Script=Han}/u.test(track.title))result.englishOriginal=track.title;
    const candidates=matching.get(checked.title)||[];
    const compatible=candidates.filter(item=>canMergeCues(item,result));
    if(compatible.length===1){
      const [target]=compatible;
      target.start||=result.start;
      target.end||=result.end;
      catalogueMerges++;
      continue;
    }
    candidates.push(result);
    matching.set(checked.title,candidates);
    ordered.push(result);
  }
  const bilingual=mergeBilingualTracks(ordered,session.composer);
  const englishTracks=bilingual.tracks.map(englishProgramTrack);
  return {session,unassigned,review,excluded,...bilingual,tracks:englishTracks,merged:bilingual.merged+catalogueMerges,corrected};
}
