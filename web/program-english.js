import {filenameFor,outputFilenameValid} from './script.js';
import {catalogueItem,musicKey} from './program-bilingual.js';
import {composerCredit,verifiedProgramWork} from './program-credits.js';
import {compact,matchWorks,numeral,parseWorkQuery,standardComposer,titleFor} from './work-catalog.js';

const chinese=/\p{Script=Han}/u;
const formats=[
  [/钢琴协奏曲|钢协|piano concerto/iu,'piano concerto'],
  [/小提琴协奏曲|violin concerto/iu,'violin concerto'],
  [/钢琴奏鸣曲|piano sonata|奏鸣曲/iu,'piano sonata'],
  [/交响曲|symphony/iu,'symphony'],
  [/英国组曲|english suite/iu,'english suite'],
  [/夜曲|nocturne/iu,'nocturne'],
  [/变奏曲|variations/iu,'variations'],
  [/前奏曲|prelude/iu,'prelude'],
  [/即兴曲|impromptu/iu,'impromptu']
];
const family=title=>formats.find(([pattern])=>pattern.test(title))?.[1]||'';
const ordinal=title=>{
  const match=title.match(/第\s*([一二三四五六七八九十两\d]+)\s*(?:号|首|(?=钢琴|交响曲|奏鸣曲|英国组曲))/u);
  return match?numeral(match[1]):'';
};
const composerName=text=>composerCredit(text)?.composer||standardComposer(text);

export function englishProgramTrack(track){
  const original=track.title.trim().normalize('NFKC');
  if(!chinese.test(original))return track;
  const credit=composerCredit(original);
  const title=credit?.title||original;
  const query=parseWorkQuery(title,credit?.composer||track.composer);
  const explicit=credit?.composer||standardComposer(title);
  const given=track.composer.trim();
  const composer=explicit||composerName(given);
  const trusted=verifiedProgramWork(title,explicit||composer);
  if(trusted)return {...track,title:trusted.title,composer:trusted.composer,englishOriginal:track.title};
  const needsReview={...track,englishReview:true};
  if(!explicit&&given&&!composer)return needsReview;
  if(query.movementText&&!query.movement)return needsReview;
  const markerIndex=original.indexOf(query.movementText);
  const remainder=original.slice(markerIndex+query.movementText.length).trim();
  // A tempo or subtitle following the movement must remain visible if we cannot verify it.
  if(query.movementText&&(markerIndex<0||/[\p{L}\p{N}]/u.test(remainder)))return needsReview;

  const originalFamily=family(title);
  const inputKey=musicKey(title.replace(/\s+/gu,''));
  const number=query.index||ordinal(query.base);
  const item=catalogueItem(title);
  const aliasKey=compact(query.base);
  const candidates=matchWorks(query).filter(candidate=>{
    if(composer&&candidate.composer!==composer)return false;
    const target=parseWorkQuery(candidate.title,candidate.composer);
    const alias=candidate.aliases.some(text=>compact(text)===aliasKey);
    const targetFamily=family(candidate.title);
    const targetNumber=target.index||candidate.title.match(/\bNo\.\s*(\d+)/u)?.[1];
    const targetItem=catalogueItem(candidate.title);
    const targetKey=musicKey(candidate.title);
    if(originalFamily&&targetFamily&&originalFamily!==targetFamily)return false;
    if(inputKey&&targetKey&&inputKey!==targetKey)return false;
    if(number&&targetNumber&&number!==targetNumber)return false;
    if(item&&item!==targetItem)return false;
    const numberedWork=composer&&number&&number===targetNumber&&originalFamily===targetFamily;
    // A sonata's own number can identify it even if the opus subdivision was omitted.
    if(targetItem&&!item&&!alias&&!(numberedWork&&target.index))return false;
    if(alias)return true;
    const uniqueCatalogue=query.catalog.some(code=>/^(?:k|bwv|wab|hob|woo)\d/u.test(code));
    const opusAndGenre=composer&&query.catalog.length&&originalFamily&&originalFamily===targetFamily;
    return !!(uniqueCatalogue||opusAndGenre||numberedWork);
  }).map(candidate=>({candidate,title:titleFor(candidate,query)})).filter(match=>match.title);
  if(candidates.length!==1)return needsReview;
  const [match]=candidates;
  return {...track,title:match.title,composer:match.candidate.composer,englishOriginal:track.title};
}

export function batchTrackFilenames(tracks,{composer='',extension}){
  const rule='{Track} - {Title}';
  const key=name=>name.normalize('NFC').toLowerCase();
  const names=new Map(), review=new Set();
  const existing=track=>filenameFor(rule,track,{},0);
  tracks.forEach((track,index)=>{
    const original=track.tags.Title?.trim();
    if(!original)return;
    const match=englishProgramTrack({title:original,composer:track.tags.Composer?.trim()||composer});
    const title=match.title;
    if(match.englishReview||!/\p{Script=Latin}/u.test(title)||/[^\p{Script=Latin}\p{Script=Common}\p{Script=Inherited}]/u.test(title)){
      review.add(index+1);
      return;
    }
    const number=track.tags.Track?.trim()||String(index+1).padStart(2,'0');
    const stem=filenameFor(rule,{...track,filename:''},{Track:number,Title:title},index);
    if(!stem||!outputFilenameValid(stem+'.'+extension)){
      review.add(index+1);
      return;
    }
    names.set(track.id,stem);
  });

  // A skipped track keeps its current name, so recheck until no new collision is exposed.
  let rejected;
  do{
    const counts=new Map();
    for(const track of tracks){
      const name=key(names.get(track.id)||existing(track));
      if(name)counts.set(name,(counts.get(name)||0)+1);
    }
    rejected=false;
    tracks.forEach((track,index)=>{
      const proposed=names.get(track.id);
      if(proposed&&counts.get(key(proposed))>1){
        names.delete(track.id);
        review.add(index+1);
        rejected=true;
      }
    });
  }while(rejected);

  const replaced=tracks.filter(track=>track.filename?.trim()&&names.has(track.id)&&track.filename!==names.get(track.id)).length;
  const preserved=tracks.filter(track=>track.filename?.trim()&&!names.has(track.id)).length;
  return {names,review:[...review].sort((a,b)=>a-b),preserved,replaced};
}
