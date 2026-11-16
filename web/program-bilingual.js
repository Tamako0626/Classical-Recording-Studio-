import {matchWorks,parseWorkQuery} from './work-catalog.js';
import {timeSeconds} from './script.js';

const han=/\p{Script=Han}/u;
const latin=/\p{Script=Latin}{2,}/u;
const compact=value=>value.normalize('NFKD').replace(/\p{Diacritic}|[^\p{L}\p{N}]/gu,'').toLowerCase();
const localizedWork=/^(?:月光|船歌|童年情景|英雄|田园|命运|未完成(?:交响曲)?|悲怆(?:交响曲)?|欢乐岛)$/u;
const movementAtEnd=/(?:^|[\s:：–—])(?:([IVX]{1,5})[.、)]\s*\S|(?:movement|mvt\.?)\s*(\d+|[IVX]{1,5})\b)/iu;
const romans=['','I','II','III','IV','V','VI','VII','VIII','IX','X'];
const catalogueNumber=/(?:^|[^a-z])(?:op\.?|bwv|kv?\.?|hob\.?|wab|woo|d\.|l\.)\s*\d+[a-z]?\s*[,，-]?\s*(?:no\.?|n[º°]|第)\s*(\d+)/iu;
const workNumber=/\b(?:no\.?|n[º°])\s*(\d+)\b|第\s*([一二三四五六七八九十\d]+)\s*(?:号|首)/iu;
const keyPattern=/\b([a-g])(?:[-\s]*(sharp|flat)|\s*([#♯♭]))?\s+(major|minor)\b|([升降])?\s*([a-g])\s*([#♯♭升降])?\s*(大|小)调/iu;
const monolingual=value=>han.test(value)?'zh':latin.test(value)?'en':'';

export function musicKey(title){
  const match=title.match(keyPattern);
  if(!match)return '';
  const [,english,word,symbol,englishMode,prefix,chinese,accidental,mode]=match;
  const modifier=word||symbol||prefix||accidental||'';
  const pitch=modifier==='升'||/^[#♯]$/.test(modifier)?'sharp':modifier==='降'||modifier==='♭'?'flat':modifier.toLowerCase();
  return (english||chinese).toLowerCase()+pitch+(mode?(mode==='大'?'major':'minor'):englishMode.toLowerCase());
}

export function catalogueItem(title){
  const normalized=title.normalize('NFKC')
    .replace(/作品(?:号)?\s*(?=\d)/gu,'Op. ')
    .replace(/第\s*(\d+)\s*[号首]/gu,'No. $1');
  return normalized.match(catalogueNumber)?.[1]||normalized.match(/\bop\.?\s*\d+\s*[-/]\s*(\d+)/iu)?.[1]||'';
}

function describe(title,composer){
  const query=parseWorkQuery(title,composer);
  const match=title.match(movementAtEnd);
  const movement=query.movement||(match?(romans[Number(match[2])]|| (match[1]||match[2]).toUpperCase()):'');
  const normalizedNumber=title.match(workNumber);
  const item=catalogueItem(title);
  const key=musicKey(title);
  const family=query.family||[
    [/夜曲|nocturne/iu,'nocturne'],[/玛祖卡|mazurka/iu,'mazurka'],
    [/练习曲|[ée]tude/iu,'etude'],[/前奏曲|prelude/iu,'prelude'],
    [/叙事曲|ballade/iu,'ballade'],[/谐谑曲|scherzo/iu,'scherzo'],
    [/圆舞曲|waltz/iu,'waltz'],[/幻想曲|fantas[yi]/iu,'fantasia'],
    [/组曲|suite/iu,'suite'],[/变奏曲|variations/iu,'variations']
  ].find(([pattern])=>pattern.test(title))?.[1]||'';
  const known=matchWorks(query).filter(candidate=>{
    if(query.catalog.length&&item!==catalogueItem(candidate.title))return false;
    const sameTitle=compact(query.base)===compact(candidate.title);
    if(sameTitle)return true;
    if(query.catalog.length)return true;
    if(query.index&&query.family&&query.artist)return true;
    return candidate.aliases.some(alias=>compact(alias)===compact(query.base));
  });
  return {query,family,movement,item,key,number:query.index||normalizedNumber?.[1]||'',known,composer:query.artist||compact(composer)};
}

function sameWork(a,b){
  if(a.movement!==b.movement)return false;
  if(a.composer&&b.composer&&a.composer!==b.composer)return false;
  if(a.family&&b.family&&a.family!==b.family)return false;
  if(a.number&&b.number&&a.number!==b.number)return false;
  if(a.key&&b.key&&a.key!==b.key)return false;
  if(a.item!==b.item&&(a.item&&b.item||a.query.catalog.length&&b.query.catalog.length))return false;
  const {catalog:ac}=a.query,{catalog:bc}=b.query;
  const shared=ac.filter(id=>bc.includes(id));
  const conflict=ac.some(id=>bc.some(other=>id.replace(/\d.*/u,'')===other.replace(/\d.*/u,'')&&id!==other));
  if(conflict)return false;
  // Opus numbers repeat across composers. Independent catalogues such as BWV and K are stronger identifiers.
  if(shared.some(id=>/^(?:bwv|k|wab|hob|woo)\d/u.test(id)))return true;
  const identities=new Set(a.known.map(c=>c.title).filter(title=>b.known.some(c=>c.title===title)));
  if(identities.size===1)return true;
  if(a.item!==b.item)return false;
  if(!a.composer||a.composer!==b.composer)return false;
  if(a.family&&a.family===b.family&&a.number&&a.number===b.number&&(!ac.length||!bc.length))return true;
  return shared.length>0&&!!(a.key&&a.key===b.key||a.item&&a.item===b.item||a.family&&a.family===b.family&&(!a.number||!b.number||a.number===b.number));
}

function splitBilingualTitle(title){
  if(!han.test(title)||!latin.test(title))return null;
  const parts=title.split(/\s*[|｜／]\s*|\s+\/\s+|\s+[–—]\s+/u);
  const brackets=title.match(/^(.+?)\s*[（(]([^()（）]+)[）)]$/u);
  const split=parts.length===2?parts:brackets?.slice(1);
  if(!split)return null;
  const [first,second]=split.map(piece=>piece.trim());
  if(han.test(first)&&latin.test(second)&&!han.test(second))return [first,second];
  if(han.test(second)&&latin.test(first)&&!han.test(first))return [second,first];
  return null;
}

function sameCues(a,b){
  if(['start','end'].some(key=>a[key]&&b[key]&&timeSeconds(a[key])!==timeSeconds(b[key])))return false;
  const start=timeSeconds(a.start||b.start||'');
  const end=timeSeconds(a.end||b.end||'');
  return start===null||end===null||end>start;
}

export function mergeBilingualTracks(tracks,sessionComposer=''){
  let merged=0;
  const rows=tracks.map(track=>{
    const hint=track.composer||sessionComposer;
    const inline=splitBilingualTitle(track.title);
    if(inline&&sameWork(describe(inline[0],hint),describe(inline[1],hint))){
      merged++;
      track={...track,title:inline[1]};
    }
    const language=monolingual(track.title);
    return {track,language,info:language?describe(track.title,hint):null,matches:[]};
  });
  const chinese=rows.filter(row=>row.language==='zh');
  const english=rows.filter(row=>row.language==='en');
  for(const zh of chinese){
    for(const en of english){
      if(!sameCues(zh.track,en.track)||!sameWork(zh.info,en.info))continue;
      zh.matches.push(en);
      en.matches.push(zh);
    }
  }

  const taken=new Set();
  const output=[];
  for(const row of rows){
    if(taken.has(row))continue;
    const [other]=row.matches;
    if(row.matches.length!==1||other.matches.length!==1){
      output.push(row.track);
      continue;
    }
    const en=row.language==='en'?row:other;
    const zh=row.language==='zh'?row:other;
    const composer=en.info.query.artist||zh.info.query.artist||en.track.composer||zh.track.composer;
    output.push({...en.track,composer,start:en.track.start||zh.track.start,end:en.track.end||zh.track.end});
    taken.add(other);
    merged++;
  }
  return {tracks:output,merged};
}

export function isBilingualWorkTitle(title){
  return localizedWork.test(title)||!!splitBilingualTitle(title);
}
