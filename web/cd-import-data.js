import {parseProgram} from './program-parser.js';
import {filenameFor} from './script.js';

const leaf=value=>value.split(/[\\/]/u).pop().trim();
const cdNumber=value=>String(value).padStart(2,'0');
const order=new Intl.Collator(undefined,{numeric:true,sensitivity:'base'});
export const sortCdFiles=files=>files.slice().sort((a,b)=>order.compare(a.name,b.name));

export function decodeCue(buffer){
  const bytes=new Uint8Array(buffer);
  if(bytes[0]===255&&bytes[1]===254)return new TextDecoder('utf-16le').decode(bytes);
  if(bytes[0]===254&&bytes[1]===255)return new TextDecoder('utf-16be').decode(bytes);
  try{return new TextDecoder('utf-8',{fatal:true}).decode(bytes);}
  catch{return new TextDecoder('gb18030').decode(bytes);}
}

function frameTime(frames){
  const ms=Math.round(frames*1000/75);
  const hour=Math.floor(ms/3600000);
  const minute=Math.floor(ms/60000)%60;
  const second=Math.floor(ms/1000)%60;
  return [hour,minute,second].map(cdNumber).join(':')+'.'+String(ms%1000).padStart(3,'0');
}

export function parseCue(text,files){
  const album={};
  const tracks=[];
  let reference='',track=null,skipping=false;
  for(const line of text.split(/\r?\n/u).map(line=>line.trim()).filter(Boolean)){
    const file=line.match(/^FILE\s+(?:"([^"]+)"|(.+?))\s+\w+$/iu);
    if(file){reference=leaf(file[1]||file[2]);track=null;skipping=false;continue;}
    const start=line.match(/^TRACK\s+(\d{1,2})\s+(\S+)$/iu);
    if(start){
      skipping=start[2].toUpperCase()!=='AUDIO';
      track=skipping?null:{number:Number(start[1]),reference};
      if(track)tracks.push(track);
      continue;
    }
    if(skipping)continue;
    const metadata=line.match(/^(TITLE|PERFORMER|SONGWRITER|REM\s+(?:DATE|COMPOSER))\s+(.+)$/iu);
    if(metadata){
      const key=metadata[1].toUpperCase().replace(/\s+/gu,' ');
      const target=track||album;
      const value=metadata[2].replace(/^"(.*)"$/u,'$1').trim();
      if(value)target[{'TITLE':'title','PERFORMER':'artist','SONGWRITER':'composer','REM COMPOSER':'composer','REM DATE':'date'}[key]]=value;
      continue;
    }
    if(!track||!/^INDEX\s+01(?:\s|$)/iu.test(line))continue;
    const cue=line.match(/^INDEX\s+01\s+(\d+):(\d{2}):(\d{2})$/iu);
    if(!cue||Number(cue[2])>59||Number(cue[3])>74)throw Error('第 '+track.number+' 轨的 INDEX 01 时间无效。');
    if(track.frames!==undefined)throw Error('第 '+track.number+' 轨有多个 INDEX 01。');
    track.frames=(Number(cue[1])*60+Number(cue[2]))*75+Number(cue[3]);
  }
  if(!tracks.length)throw Error('CUE 中没有找到 AUDIO 音轨。');
  const numbers=new Set();
  const previous=new Map();
  for(const row of tracks){
    if(!row.number||numbers.has(row.number))throw Error('CUE 的轨号缺失或重复。');
    numbers.add(row.number);
    if(!row.reference||!Number.isSafeInteger(row.frames))throw Error('第 '+row.number+' 轨缺少 FILE 或 INDEX 01。');
    const reference=row.reference.toLowerCase();
    let matches=files.filter(file=>file.name.toLowerCase()===reference);
    if(!matches.length){
      const stem=name=>name.toLowerCase().replace(/\.[^.]+$/u,'');
      matches=files.filter(file=>stem(file.name)===stem(reference));
    }
    if(matches.length!==1)throw Error('请导入 CUE 引用的音频文件：'+row.reference+'。');
    const file=matches[0];
    const prior=previous.get(file.name);
    if(prior&&row.frames<=prior.frames)throw Error('同一音频文件里的 INDEX 01 必须依次递增。');
    const start=frameTime(row.frames);
    if(prior)prior.end=start;
    Object.assign(row,{source:file.name,start,end:''});
    previous.set(file.name,row);
  }
  return {album,tracks};
}

const albumLabels={
  title:/^(?:album(?: (?:title|name))?|release title|record title|专辑(?:名|名称|标题)?|唱片(?:名|名称|标题))$/iu,
  artist:/^(?:album (?:artist|performer)s?|artist|performers?|专辑(?:演出者|表演者|艺术家|演奏家)|演出(?:者|家)|演奏(?:者|家)|艺术家)$/iu,
  composer:/^(?:album composers?|composers?|专辑作曲家|作曲(?:家)?)$/iu,
  date:/^(?:album (?:date|year)|(?:release|recording) (?:date|year)|date|year|专辑(?:日期|年份)|发行(?:日期|年份|年)|录音(?:日期|年份)|日期|年份)$/iu
};
const stripMarkup=raw=>raw.trim().replace(/^(?:[-*•]\s+|#{1,6}\s+)/u,'').replace(/\*\*|__/gu,'');
const unknownAlbumValue=/^(?:unknown|n\/?a|not (?:specified|provided)|未知|未提供|不详|—)$/iu;
function albumMetadata(text){
  const album={};
  let sawTrack=false;
  for(const raw of text.split(/\r?\n/u)){
    const line=stripMarkup(raw);
    if(/^(?:track\s*)?\d{1,2}(?:[.、.)：:–—-]|\s+)/iu.test(line)){sawTrack=true;continue;}
    const parts=line.match(/^([^:：]+)[:：]\s*(.+)$/u);
    if(!parts)continue;
    const label=parts[1].normalize('NFKC').trim().replace(/\s+/gu,' ');
    if(sawTrack&&!/^(?:album |release |recording |专辑|唱片|发行|录音)/iu.test(label))continue;
    const key=Object.keys(albumLabels).find(key=>albumLabels[key].test(label));
    if(!key||album[key])continue;
    const value=parts[2].trim().replace(/^["“](.+)["”]$/u,'$1').trim().slice(0,280);
    if(!value||unknownAlbumValue.test(value))continue;
    if(key==='date'){
      const date=parseProgram('Date: '+value).session.date||value.normalize('NFKC').match(/^\d{4}$/u)?.[0];
      if(date)album.date=date;
    }else album[key]=value;
  }
  return album;
}

export function readCdText(text){
  const numbered=new Map(),issues=[];
  for(const raw of text.split(/\r?\n/u)){
    const line=stripMarkup(raw).normalize('NFKC');
    const match=line.match(/^(?:track\s*)?(\d{1,2})(?:\s*[.、.)：:–—-]\s*|\s+)(\S.*)$/iu);
    if(!match)continue;
    const number=Number(match[1]);
    if(!number||numbered.has(number)){
      issues.push('轨号 '+number+' 重复或无效，请核对文字。');
      continue;
    }
    const title=match[2].replace(/\s+\(?\d{1,3}:[0-5]\d\)?\s*$/u,'').trim();
    if(!title||!/\p{L}/u.test(title))continue;
    const polished=parseProgram('1. '+title).tracks[0];
    numbered.set(number,{title:polished?.title||title,composer:polished?.composer||''});
  }
  const {session,tracks}=parseProgram(text);
  const labeledYear=text.match(/^(?:年份|发行年|year|date)\s*[:：]\s*(\d{4})\s*$/imu)?.[1];
  const album={title:session.name||'',artist:session.performer||session.conductor||'',composer:session.composer||'',date:session.date||labeledYear||'',...albumMetadata(text)};
  for(const key of Object.keys(album))if(unknownAlbumValue.test(album[key]))album[key]='';
  const ordered=numbered.size?[]:tracks;
  return {album,numbered,ordered,issues};
}

export function cdFilename(row,index){
  if(!row.title?.trim())return '';
  return filenameFor('{Track} - {Title}',{source:row.source},{Track:cdNumber(row.number),Title:row.title},index);
}
