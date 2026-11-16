const composers = [
  ['Ludwig van Beethoven','贝多芬','贝','beethoven'],['Wolfgang Amadeus Mozart','莫扎特','莫','mozart'],
  ['Frédéric Chopin','肖邦','肖','chopin'],['Johann Sebastian Bach','巴赫','bach'],
  ['Robert Schumann','舒曼','schumann'],['Franz Schubert','舒伯特','schubert'],
  ['Claude Debussy','德彪西','debussy'],['Franz Liszt','李斯特','liszt'],
  ['Pyotr Ilyich Tchaikovsky','柴可夫斯基','柴','tchaikovsky'],['Sergei Rachmaninoff','拉赫玛尼诺夫','拉','rachmaninov','rachmaninoff'],
  ['Gustav Mahler','马勒','马','mahler'],['Anton Bruckner','布鲁克纳','布','bruckner'],
  ['Johannes Brahms','勃拉姆斯','勃','brahms'],['Maurice Ravel','拉威尔','ravel'],
  ['Felix Mendelssohn','门德尔松','mendelssohn'],['Richard Wagner','瓦格纳','wagner']
];
const repertoire = [
  [0,'Symphony No. 3 in E-flat major, Op. 55, Eroica','贝三|英雄交响曲|贝多芬第三交响曲'],
  [0,'Symphony No. 5 in C minor, Op. 67','贝五|命运'],
  [0,'Symphony No. 6 in F major, Op. 68, Pastoral','贝六|田园'],
  [0,'Symphony No. 7 in A major, Op. 92','贝七'],
  [0,'Symphony No. 9 in D minor, Op. 125, Choral','贝九|合唱交响曲'],
  [0,'Piano Sonata No. 6 in F major, Op. 10 No. 2','贝多芬第六奏鸣曲|贝多芬第六钢琴奏鸣曲|贝多芬第6首钢琴奏鸣曲'],
  [0,'Piano Sonata No. 7 in D major, Op. 10 No. 3','贝多芬第七奏鸣曲|贝多芬第七钢琴奏鸣曲'],
  [0,'Piano Sonata No. 14 in C-sharp minor, Op. 27 No. 2, Moonlight','月光奏鸣曲|贝多芬月光|月光'],
  [0,'Piano Sonata No. 30 in E major, Op. 109','贝多芬第三十钢琴奏鸣曲'],
  [1,'Piano Sonata No. 11 in A major, K. 331','土耳其进行曲奏鸣曲'],
  [1,'Piano Sonata No. 8 in A minor, K. 310','莫扎特第八钢琴奏鸣曲'],
  [1,'Piano Sonata No. 14 in C minor, K. 457','莫扎特第十四钢琴奏鸣曲'],
  [1,'Fantasia in C minor, K. 475','莫扎特幻想曲'],
  [1,'Piano Concerto No. 24 in C minor, K. 491','莫扎特第二十四钢琴协奏曲'],
  [2,'Piano Concerto No. 1 in E minor, Op. 11','肖协1|肖一钢协'],
  [2,'Piano Concerto No. 2 in F minor, Op. 21','肖协2|肖二钢协'],
  [2,'Mazurkas, Op. 7','肖邦Op7|肖邦玛祖卡Op.7'],
  [2,'Barcarolle in F-sharp major, Op. 60','肖邦船歌|船歌'],
  [2,'Nocturne in B major, Op. 62 No. 1','肖邦夜曲Op62-1'],
  [3,'English Suite No. 2 in A minor, BWV 807','巴赫第二英国组曲'],
  [4,'Piano Concerto in A minor, Op. 54','舒曼钢协'],
  [4,'Kinderszenen, Op. 15','童年情景'],
  [4,'Fantasie in C major, Op. 17','舒曼C大调幻想曲'],
  [4,'Symphonic Etudes, Op. 13','交响练习曲'],
  [5,'Symphony No. 8 in B minor, D. 759, Unfinished','舒伯特未完成|未完成交响曲'],
  [5,'Symphony No. 9 in C major, D. 944, The Great','舒九|伟大交响曲'],
  [6,'Suite bergamasque: III. Clair de lune','德彪西月光|月光|clair de lune'],
  [6,'L’isle joyeuse, L. 106','欢乐岛'],
  [8,'Symphony No. 6 in B minor, Op. 74, Pathétique','柴六|悲怆交响曲'],
  [9,'Piano Concerto No. 2 in C minor, Op. 18','拉二钢协|拉赫二协|拉赫玛尼诺夫二协'],
  [10,'Symphony No. 2 in C minor, Resurrection','马二|复活交响曲'],
  [11,'Symphony No. 7 in E major, WAB 107','布七'],
  [12,'Piano Concerto No. 2 in B-flat major, Op. 83','勃拉姆斯二协']
];
const number = /^[一二三四五六七八九十两零〇]+$/u;
export function numeral(s) {
  if(!number.test(s))return s;
  const digit=c=>({'一':1,'二':2,'三':3,'四':4,'五':5,'六':6,'七':7,'八':8,'九':9,'两':2,'零':0,'〇':0})[c];
  const parts=s.split('十');
  if(parts.length===1)return [...s].map(digit).join('');
  return String((digit(parts[0])||1)*10+(digit(parts[1])||0));
}
const fold=text=>String(text??'').normalize('NFKD').replace(/\p{Diacritic}/gu,'').toLowerCase();
export const compact=text=>fold(text).replace(/[^\p{L}\p{N}]/gu,'');
const movementPattern=/(?:第?([一二三四五六七八九十两\d]+)乐章|(?:^|[\s:–-])(?:mvt\.?|movement)\s*([1-9ivx]+)\b)/iu;
const romans=['','I','II','III','IV','V','VI','VII','VIII','IX','X'];
const chapterInTitle=/(?:\s*[:–-]\s*|\s+)(I{1,3}|IV|V|VI{1,3}|IX|X)\.\s*\S/i;
function normalizedText(input) {
  return fold(input)
    .replace(/第?([一二三四五六七八九十两零〇]+)(?=号|首|钢琴|钢协|交响曲|协奏曲|奏鸣曲)/gu,(_,n)=>numeral(n)+' ')
    .replace(/钢琴协奏曲|钢协|(?<=\d)协/gu,' piano concerto ').replace(/交响曲/gu,' symphony ')
    .replace(/钢琴奏鸣曲|奏鸣曲/gu,' piano sonata ').replace(/小提琴协奏曲/gu,' violin concerto ')
    .replace(/玛祖卡/gu,' mazurka ').replace(/夜曲/gu,' nocturne ').replace(/号|第/gu,' ')
    .replace(/(?:作品(?:编号|号)?|第)\s*(?=\d)/gu,(label)=>label.startsWith('作品')?' op ':' ')
    .replace(/\b(kv?|op|bwv|hob|wab|d|s|l|woo)\s*\.?\s*(?=\d)/g,'$1 ')
    .replace(/\bkv\b/g,'k').replace(/\bno\s*\.?\s*(?=\d)/g,' ')
    .replace(/[^\p{L}\p{N}]+/gu,' ').trim();
}
function findComposer(text, allowShort) {
  const value=fold(text);
  return composers.find(([full,...names])=>names.some(alias=>{
    if(/^[a-z]+$/.test(alias))return new RegExp('\\b'+alias+'\\b').test(value);
    if(alias.length===1)return allowShort&&new RegExp('^'+alias+'(?=(?:协|钢协|[\\d一二三四五六七八九十]))','u').test(value);
    return value.includes(alias);
  })||value.includes(fold(full)));
}
export const standardComposer=text=>findComposer(text,false)?.[0]||'';
export function composerFromCredit(text){
  const name=compact(text);
  return composers.find(([full,...aliases])=>[full,...aliases.filter(alias=>alias.length>1)].some(alias=>compact(alias)===name))?.[0]||'';
}
export function parseWorkQuery(raw,hint=''){
  const input=String(raw).normalize('NFKC').trim();
  const movement=input.match(movementPattern);
  const movementKey=movement?.[1]?romans[Number(numeral(movement[1]))]:movement?.[2]?(romans[Number(movement[2])]||movement[2].toUpperCase()):'';
  const base=movement?input.replace(movement[0],' ').trim():input;
  const identified=findComposer(base,true);
  const composer=identified||findComposer(hint,false);
  const aliases=composer?[...composer].sort((a,b)=>b.length-a.length):[];
  let rest=fold(base);
  for(const alias of aliases){
    const key=fold(alias);
    if(key.length===1)rest=rest.replace(new RegExp('^'+key+'(?=[\\d一二三四五六七八九十协钢])','u'),'');
    if(key.length>1)rest=rest.replaceAll(key,' ');
  }
  rest=rest.replace(/^协(?=\s*\d)/u,'钢协 ');
  const numberedSymphony=/^[一二三四五六七八九十两\d]+$/u.test(rest.trim())&&identified&&base.trim().length<=3;
  if(numberedSymphony)rest='symphony '+numeral(rest.trim());
  const work=normalizedText(rest);
  const artist=composer?.[0]||'';
  const catalog=[...work.matchAll(/\b(k|op|bwv|hob|wab|d|s|l|woo)\s+(\d+[a-z]?)\b/g)].map(m=>m[1]+m[2]);
  const family=work.match(/\b(piano concerto|violin concerto|piano sonata|symphony)\b/);
  const index=family?work.match(new RegExp(family[0]+'\\s+(\\d+)'))?.[1]||work.match(new RegExp('(\\d+)\\s+'+family[0]))?.[1]:'';
  const terms=work.split(' ').filter(Boolean);
  const online=family&&index?family[0]+' No. '+index+(catalog.length?' '+catalog.join(' '):''):work;
  return {input,base,artist,movement:movementKey||'',movementText:movement?.[0].trim()||'',catalog,family:family?.[0]||'',index:index||'',terms,online};
}
export function matchWorks(query,projects=[]){
  const local=repertoire.map(([i,title,aliases])=>({title,composer:composers[i][0],aliases:aliases.split('|'),source:'内置曲目表'}));
  const workspace=projects.flatMap(p=>(p.tracks||[]).filter(t=>t.tags?.Title).map(t=>({title:t.tags.Title,composer:t.tags.Composer||p.composer||'',aliases:[],source:'录音项目曲目'})));
  const key=compact(query.base);
  return [...local,...workspace].map(candidate=>{
    if(query.artist&&candidate.composer&&!fold(candidate.composer).includes(fold(query.artist))&&findComposer(candidate.composer,false)?.[0]!==query.artist)return null;
    const title=normalizedText(candidate.title);
    const aliases=candidate.aliases.map(compact);
    const candidateCatalog=[...title.matchAll(/\b(k|op|bwv|hob|wab|d|s|l|woo)\s+(\d+[a-z]?)\b/g)].map(m=>m[1]+m[2]);
    if(query.catalog.some(code=>!candidateCatalog.includes(code)))return null;
    const isAlias=aliases.some(item=>key===item||(key.length>=4&&item.includes(key)));
    if(query.family&&!title.includes(query.family)&&!isAlias)return null;
    if(query.index&&!new RegExp(query.family+'\\s+'+query.index+'\\b').test(title)&&!isAlias)return null;
    const hits=query.terms.filter(term=>title.split(' ').includes(term)).length;
    if(!isAlias&&!hits&&!query.catalog.length)return null;
    if(!/\p{Script=Latin}/u.test(candidate.title)||/\p{Script=Han}/u.test(candidate.title))return null;
    if(!isAlias&&!query.family&&!query.catalog.length&&hits<query.terms.length)return null;
    return {...candidate,score:(isAlias?24:0)+query.catalog.length*14+(query.index?10:0)+hits+(candidate.source==='内置曲目表'?0.2:0)};
  }).filter(Boolean).sort((a,b)=>b.score-a.score);
}
export function titleFor(candidate,query){
  if(!query.movementText)return candidate.title;
  const parts=candidate.title.match(chapterInTitle);
  if(parts)return parts[1].toUpperCase()===query.movement?candidate.title:'';
  return candidate.title+' – '+(query.movement?'Movement '+query.movement:query.movementText);
}
export function mergeWorks(local,remote,query){
  const seen=new Set();
  return [...local,...remote].map(candidate=>({...candidate,title:titleFor(candidate,query)})).filter(c=>{
    if(!c.title)return false;
    const key=compact(c.title)+'|'+compact(c.composer);
    if(seen.has(key))return false;
    seen.add(key);
    return true;
  }).slice(0,10);
}
