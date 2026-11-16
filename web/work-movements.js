import {catalogueItem,musicKey} from './program-bilingual.js';
import {englishProgramTrack} from './program-english.js';
import {composerFromCredit,compact,numeral,parseWorkQuery} from './work-catalog.js';
import {composerCredit,verifiedProgramWork} from './program-credits.js';
import {blankTrack} from './seed.js';

const works=[
  ['Ludwig van Beethoven','Symphony No. 3 in E-flat major, Op. 55, Eroica','https://www.laphil.com/works/symphony-no-3-in-e-flat-major-op-55-eroica',
    ['Allegro con brio','Marcia funebre: Adagio assai','Scherzo: Allegro vivace','Finale: Allegro molto']],
  ['Ludwig van Beethoven','Piano Sonata No. 6 in F major, Op. 10 No. 2','https://www.pianolibrary.org/composers/beethoven/sonata-6-10-2/',
    ['Allegro','Allegretto','Presto']],
  ['Ludwig van Beethoven','Piano Sonata No. 7 in D major, Op. 10 No. 3','https://www.pianolibrary.org/composers/beethoven/sonata-7-10-3/',
    ['Presto','Largo e mesto','Menuetto: Allegro','Rondo: Allegro']],
  ['Ludwig van Beethoven','Piano Sonata No. 14 in C-sharp minor, Op. 27 No. 2, Moonlight','https://katalog.beethoven.de/cgi-bin/biblio/kat_en.pl?reccheck=90732&t_show=x',
    ['Adagio sostenuto','Allegretto','Presto agitato']],
  ['Ludwig van Beethoven','Piano Sonata No. 30 in E major, Op. 109','https://www.pianolibrary.org/composers/beethoven/sonata-30-109/',
    ['Vivace, ma non troppo – Adagio espressivo','Prestissimo','Gesangvoll, mit innigster Empfindung']],
  ['Robert Schumann','Fantasiestücke, Op. 12','https://catalogue.bnf.fr/ark:/12148/cb13918915s',
    ['Des Abends','Aufschwung','Warum?','Grillen','In der Nacht','Fabel','Traumes Wirren','Ende vom Lied']],
  ['Robert Schumann','Symphonic Etudes, Op. 13','https://imslp.org/wiki/Symphonic_Etudes_Op.13_(Schumann,_Robert)',
    ['Theme: Andante','Etude I (Variation 1)','Etude II (Variation 2)','Etude III: Vivace',
      'Etude IV (Variation 3)','Etude V (Variation 4)','Etude VI (Variation 5)','Etude VII (Variation 6)',
      'Etude VIII (Variation 7)','Etude IX: Presto possibile','Etude X (Variation 8)','Etude XI (Variation 9)',
      'Etude XII (Finale): Allegro brillante','Posthumous Variation I','Posthumous Variation II',
      'Posthumous Variation III','Posthumous Variation IV','Posthumous Variation V'],13],
  ['Robert Schumann','Kinderszenen, Op. 15','https://www.jpc.de/jpcng/classic/detail/-/art/Robert-Schumann-1810-1856-Geistervariationen-1854/hnum/9715317',
    ['Von fremden Ländern und Menschen','Kuriose Geschichte','Hasche-Mann','Bittendes Kind','Glückes genug','Wichtige Begebenheit','Träumerei','Am Kamin','Ritter vom Steckenpferd','Fast zu ernst','Fürchtenmachen','Kind im Einschlummern','Der Dichter spricht']],
  ['Maurice Ravel','Gaspard de la nuit','https://catalogue.bnf.fr/ark:/12148/cb139177317',
    ['Ondine','Le Gibet','Scarbo']],
  ['Wolfgang Amadeus Mozart','Piano Sonata No. 11 in A major, K. 331','https://s9.imslp.org/files/imglnks/usimg/8/86/IMSLP858445-PMLP1846-Mozart11Full060823.pdf',
    ['Andante grazioso (Theme and Variations)','Menuetto and Trio','Alla turca: Allegretto']],
  ['Wolfgang Amadeus Mozart','Piano Sonata No. 8 in A minor, K. 310','https://www.halleonard.com/bin/MiscEducationalKeyboardMozartslides2024.pdf',
    ['Allegro maestoso','Andante cantabile con espressione','Presto']],
  ['Wolfgang Amadeus Mozart','Piano Sonata No. 14 in C minor, K. 457','https://www.pianolibrary.org/composers/mozart/sonata-457/',
    ['Molto allegro','Adagio','Allegro assai']],
  ['Johann Sebastian Bach','English Suite No. 2 in A minor, BWV 807','https://www.bachvereniging.nl/en/bwv/bwv-807',
    ['Prelude','Allemande','Courante','Sarabande','Bourrée I and II','Gigue']],
  ['Frédéric Chopin','Piano Concerto No. 2 in F minor, Op. 21','https://londonso-assets.s3.amazonaws.com/uploads/2026/02/1215-02-Chopin-Web.pdf',
    ['Maestoso','Larghetto','Allegro vivace']],
  ['Pyotr Ilyich Tchaikovsky','Symphony No. 6 in B minor, Op. 74, Pathétique','https://en.tchaikovsky-research.net/pages/Symphony_No._6',
    ['Adagio – Allegro non troppo','Allegro con grazia','Allegro molto vivace','Adagio lamentoso']]
].map(([composer,title,source,parts,mainCount])=>({composer,title,source,parts,mainCount,numberedTitles:mainCount!==undefined,query:parseWorkQuery(title,composer),item:catalogueItem(title)}));

const chineseGenres=[
  [/小提琴奏鸣曲/u,'violin sonata'],[/大提琴奏鸣曲/u,'cello sonata'],[/小提琴协奏曲/u,'violin concerto'],
  [/大提琴协奏曲/u,'cello concerto'],[/钢琴协奏曲|钢协/u,'piano concerto'],[/钢琴奏鸣曲/u,'piano sonata'],
  [/钢琴三重奏/u,'piano trio'],[/钢琴五重奏/u,'piano quintet'],[/弦乐四重奏/u,'string quartet'],
  [/弦乐五重奏/u,'string quintet'],[/英国组曲/u,'english suite'],[/法国组曲/u,'french suite'],
  [/交响曲/u,'symphony'],[/帕蒂塔/u,'partita'],[/组曲/u,'suite'],[/奏鸣曲/u,'sonata'],
  [/协奏曲/u,'concerto'],[/变奏曲/u,'variations'],[/歌剧/u,'opera'],[/弥撒曲?/u,'mass']
];

export function movementSearch(title,credit=''){
  const inline=composerCredit(title);
  const raw=inline?.title||title.trim().normalize('NFKC');
  const translated=englishProgramTrack({title,composer:credit});
  const parsed=parseWorkQuery(raw,credit);
  const artist=inline?.composer||parsed.artist||credit.trim();
  const normalized=parseWorkQuery(translated.title,artist);
  const hasChinese=/\p{Script=Han}/u.test(translated.title);
  const genre=chineseGenres.find(([pattern])=>pattern.test(raw))?.[1];
  const number=raw.replace(/作品.*$/u,'').match(/第\s*([一二三四五六七八九十两\d]+)|([一二三四五六七八九十两\d]+)\s*号/u);
  const index=number?numeral(number[1]||number[2]):parsed.index;
  const codes=normalized.catalog.map(code=>code.replace(/([a-z])(?=\d)/u,'$1 ')).join(' ');
  const strong=normalized.catalog.some(code=>/^(?:bwv|k|wab|hob|woo|d)\d/u.test(code));
  if(strong)return {work:codes,artist};
  if(hasChinese&&genre&&(index||codes))return {work:[genre,index,codes].filter(Boolean).join(' '),artist};
  if(hasChinese&&codes)return {work:codes,artist};
  if(!hasChinese){
    if(normalized.family&&normalized.index)return {work:[normalized.family,normalized.index,codes].filter(Boolean).join(' '),artist};
  }
  return {work:hasChinese?raw:normalized.online||raw,artist};
}

export function findMovementList(title,credit=''){
  if(!title.trim())return null;
  const inline=composerCredit(title);
  const input=inline?.title||title;
  const originalQuery=parseWorkQuery(input,credit);
  if(originalQuery.movementText||/(?:^|[\s:–—])[IVX]{1,5}\.\s*\S/iu.test(input))return null;
  const fieldComposer=composerCredit(credit)?.composer||composerFromCredit(credit);
  const translated=englishProgramTrack({title,composer:credit});
  const verified=verifiedProgramWork(inline?.title||title,inline?.composer||fieldComposer);
  const resolved=verified?.title||translated.title;
  const query=parseWorkQuery(resolved,inline?.composer||fieldComposer||translated.composer);
  const composer=inline?.composer||query.artist||fieldComposer;
  const item=catalogueItem(input)||catalogueItem(resolved);
  const key=musicKey(input)||musicKey(resolved);
  const matches=works.filter(work=>{
    if(composer&&composer!==work.composer)return false;
    if(item&&item!==work.item)return false;
    if(key&&musicKey(work.title)&&key!==musicKey(work.title))return false;
    if(compact(work.title)===compact(resolved))return true;
    if(work.mainCount&&composer===work.composer&&!item&&
      (/^(?:op\s*13|交响练习曲)$/iu.test(query.terms.join(' '))||/^(?:symphonic etudes|etudes symphoniques|symphonische etuden)(?: op 13)?$/u.test(query.terms.join(' '))))return true;
    if(!query.catalog.every(code=>work.query.catalog.includes(code)))return false;
    // An Opus subdivision can identify a complete sonata or one piece inside a collection.
    // Use it only when the catalogue row itself describes that numbered work.
    const opusOnly=query.terms.every(term=>/^(?:op|\d+[a-z]?)$/u.test(term));
    if(item&&item===work.item&&composer&&query.catalog.length&&opusOnly)return true;
    if(item&&!work.item)return false;
    const catalogueOnly=query.catalog.length&&query.catalog.join('')===query.terms.join('');
    if(catalogueOnly)return query.catalog.some(code=>/^(?:k|bwv)\d/u.test(code)||composer)&&!work.item;
    return !!(composer&&work.query.family&&work.query.family===query.family&&query.index&&work.query.index===query.index);
  });
  return matches.length===1?matches[0]:null;
}

const romans=['I','II','III','IV','V','VI','VII','VIII','IX','X','XI','XII','XIII','XIV','XV','XVI','XVII','XVIII','XIX','XX'];

export function searchResultMovements(works){
  const groups=new Map();
  const split=title=>title.match(/^(.+):\s*([IVXLCDM]+|\d+)\.\s+(.+)$/iu);
  for(const work of works){
    const match=split(work.original||work.title)||split(work.title);
    if(!match)continue;
    const [,parent,number,name]=match;
    const order=/^\d+$/u.test(number)?Number(number):romans.indexOf(number.toUpperCase())+1;
    if(order<1||!Number.isSafeInteger(order))continue;
    const key=compact(parent)+'|'+compact(work.composer);
    if(!groups.has(key))groups.set(key,{title:split(work.title)?.[1]||parent,composer:work.composer,entries:new Map()});
    const entries=groups.get(key).entries;
    const entry={index:order-1,name:split(work.title)?.[3]||name,number,source:work.url};
    if(!entries.has(order))entries.set(order,entry);
    else if(compact(entries.get(order)?.name)!==compact(entry.name))entries.set(order,null);
  }
  return [...groups.values()].flatMap(({title,composer,entries})=>{
    const parts=[...entries].sort(([a],[b])=>a-b).map(([,entry])=>entry).filter(Boolean);
    return parts.length?[{title,composer,entries:parts}]:[];
  });
}

export function movementTitles(parent,parts){
  return parts.map(({name,index,numbered,number})=>parent+' – '+(numbered?'':(number||romans[index]||String(index+1))+'. ')+name);
}

export function expandMovementTrack(parent,titles,composer){
  return titles.map((Title,index)=>({
    ...blankTrack(),source:parent.source,
    start:index===0?parent.start:'',
    end:index===titles.length-1?parent.end:'',
    awaitingCue:index>0||!!parent.awaitingCue,
    tags:{...parent.tags,Title,Composer:composer||parent.tags.Composer,Track:''}
  }));
}

export function modelMovementList(value){
  if(!value||!Array.isArray(value.parts))throw Error('模型没有返回可用的乐章列表。');
  if(!value.parts.length)return null;
  const title=typeof value.title==='string'?value.title.trim():'';
  const composer=typeof value.composer==='string'?value.composer.trim():'';
  if(!title||title.length>230||composer.length>160||value.parts.length>150)throw Error('模型返回的作品或乐章数量超出范围。');
  const seen=new Set();
  const entries=value.parts.map((part,index)=>{
    const name=typeof part?.name==='string'?part.name.trim():'';
    const number=typeof part?.number==='string'?part.number.trim().replace(/\.$/u,''):'';
    if(!name||name.length>230||/[\x00-\x1f]/u.test(name)||number&&!/^[\p{L}\p{N}. ()-]{1,32}$/u.test(number))throw Error('模型返回的乐章名称或编号不完整。');
    const key=(number+' '+name).normalize('NFKC').toLowerCase();
    if(seen.has(key))throw Error('模型返回了重复的乐章，请重试。');
    seen.add(key);
    return {name,number,index};
  });
  return {title,composer,entries,fromModel:true};
}
