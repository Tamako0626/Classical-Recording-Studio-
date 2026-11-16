const queryTerm = value => '"'+value.replace(/["\\]/g,'\\$&')+'"';
const workId = /^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i;
let lastRequest = 0;
const cache = new Map();

async function musicBrainz(path, params) {
  const endpoint=new URL('https://musicbrainz.org/ws/2/work'+path);
  endpoint.search=new URLSearchParams({...params,fmt:'json'});
  const key=endpoint.href;
  const cached=cache.get(key);
  if(cached&&Date.now()-cached.time<3600000)return cached.data;
  const delay=Math.max(0,1100-(Date.now()-lastRequest));
  if(delay>2200)throw Error('limit');
  lastRequest=Date.now()+delay;
  if(delay)await new Promise(resolve=>setTimeout(resolve,delay));
  const response=await fetch(endpoint,{headers:{
    'Accept':'application/json',
    'User-Agent':'ClassicalRecordingStudio/1.0 (https://classical-recording-studio.beigegrass42.chatgpt.site)'
  },signal:AbortSignal.timeout(6500)});
  if(!response.ok)throw Error('upstream');
  const data=await response.json();
  if(cache.size>=100)cache.delete(cache.keys().next().value);
  cache.set(key,{time:Date.now(),data});
  return data;
}

async function lookupWorkParts(url) {
  const id=url.searchParams.get('id')||'';
  if(!workId.test(id))return Response.json({error:'作品编号无效。'},{status:400});
  try{
    const result=await musicBrainz('/'+id,{inc:'work-rels+artist-rels'});
    if(!Array.isArray(result.relations))throw Error('invalid response');
    const relations=result.relations.filter(relation=>relation['type-id']==='ca8d3642-ce5f-49f8-91f2-125d72524e6a'
      &&relation.direction==='forward'&&relation.work);
    const parts=relations.map(relation=>{
      const title=relation.work.title?.trim()||'';
      const numbered=title.match(/(?:^|[:–—]\s*)([IVXLCDM]{1,8}|\d+)\.\s+(.+)$/u);
      // Removing a work prefix must preserve subtitles such as "Rondo: Allegro".
      const parentPrefix=result.title&&title.startsWith(result.title+': ')?result.title.length+2:0;
      const name=numbered?.[2]||(parentPrefix?title.slice(parentPrefix):title);
      const order=relation['ordering-key'];
      return {name,order:Number.isSafeInteger(order)&&order>0?order:null};
    });
    const ordered=parts.length>0&&parts.every(part=>part.order!==null&&part.name)
      &&new Set(parts.map(part=>part.order)).size===parts.length;
    if(ordered)parts.sort((a,b)=>a.order-b.order);
    const numbered=ordered&&parts.every(({order},i)=>order===i+1);
    const composer=result.relations.find(relation=>relation.type==='composer')?.artist?.name||'';
    return Response.json({parts:parts.map(({name})=>name),ordered:numbered,composer});
  }catch{
    return Response.json({error:'暂时无法读取 MusicBrainz 乐章目录。请稍后重试，或粘贴节目单上的乐章。'},{status:502});
  }
}

async function searchWorks(request) {
  const user = request.headers.get('oai-authenticated-user-id');
  if (!user) return Response.json({error:'请登录后查找曲目。'},{status:401});
  const url = new URL(request.url);
  const work = (url.searchParams.get('work')||'').trim();
  const artist = (url.searchParams.get('artist')||'').trim();
  if (request.method!=='GET') return Response.json({error:'不支持此操作。'},{status:405});
  if(url.searchParams.has('id'))return lookupWorkParts(url);
  const forParts=url.searchParams.get('mode')==='parts';
  if(work.length<2||work.length>160||artist.length>120)return Response.json({works:[]});
  const clauses = [];
  const phrase=forParts?null:work.match(/^(.+?)\s+(?:(?:k|op|bwv|hob|wab|d|s|l|woo)\d+[a-z]?)(?:\s|$)/i)?.[1];
  if(phrase){
    clauses.push('(work:'+queryTerm(phrase)+' OR alias:'+queryTerm(phrase)+')');
    for(const [,prefix,number] of work.matchAll(/\b(k|op|bwv|hob|wab|d|s|l|woo)(\d+[a-z]?)\b/gi)){
      clauses.push('(work:'+queryTerm(prefix+' '+number)+' OR alias:'+queryTerm(prefix+' '+number)+')');
    }
  }else{
    const tokens=(work.match(/\b(?:k|op|bwv|hob|wab|d|s|l|woo)\s*\d+[a-z]?\b|[^\s]+/giu)||[]).slice(0,12).map(t=>t.replace(/^(k|op|bwv|hob|wab|d|s|l|woo)(?=\d)/i,'$1 '));
    for(const term of tokens)clauses.push('(work:'+queryTerm(term)+' OR alias:'+queryTerm(term)+')');
  }
  if(artist)clauses.push('artist:'+queryTerm(artist));
  const query=clauses.join(' AND ');
  try{
    const result=await musicBrainz('',{query,limit:'25'});
    if(!Array.isArray(result.works))throw Error('invalid response');
    const works=result.works.flatMap(w=>{
      if(!workId.test(w.id))return [];
      const names=[...(w.aliases||[]).filter(a=>a.locale==='en').map(a=>a.name),w.title];
      const latin=names.find(name=>typeof name==='string'&&name.length<=260&&/\p{Script=Latin}/u.test(name)&&!/\p{Script=Han}|\p{Script=Cyrillic}|\p{Script=Hiragana}|\p{Script=Katakana}/u.test(name));
      const title=latin||(forParts&&w.title?.slice(0,260));
      if(!title)return [];
      const composer=(w.relations||[]).find(r=>r.type==='composer')?.artist?.name||artist;
      const row={title,composer,source:'MusicBrainz',url:'https://musicbrainz.org/work/'+w.id};
      if(!forParts)return [row];
      const child=(w.relations||[]).some(r=>r['type-id']==='ca8d3642-ce5f-49f8-91f2-125d72524e6a'&&r.direction==='backward'&&r.attributes?.includes('movement'));
      return child?[]:[{...row,id:w.id,original:w.title,note:w.disambiguation||'',type:w.type||''}];
    });
    return Response.json({works});
  }catch{
    return Response.json({error:forParts?'当前无法访问 MusicBrainz。可稍后重试，或粘贴节目单上的乐章。':'当前无法访问 MusicBrainz。仍可使用站内候选，或稍后重试。'},{status:502});
  }
}
