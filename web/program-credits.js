const aliases = [
  ['Robert Schumann','SCHUMANN|Robert Schumann|舒曼|羅伯特舒曼'],
  ['Gabriel Fauré','Faur[eé]|Gabriel Faur[eé]|福雷|佛雷|弗雷|佛蕾|弗蕾|福蕾'],
  ['Maurice Ravel','Ravel|Maurice Ravel|拉威尔|拉威爾']
];
const creditPattern=aliases.map(([composer,pattern])=>[composer,new RegExp('^(?:'+pattern+')\\s*[,，:：.]?\\s*$','iu'),new RegExp('^(?:'+pattern+')\\s*[,，:：]\\s*(.*)$','iu')]);
const catalogue=[
  {composer:'Robert Schumann',title:'Fantasiestücke, Op. 12',match:/fantasiest[uü]cke|幻想(?:小品|曲集|曲五)|幻想小品集/iu,id:/\bop\.?\s*12\b|作品\s*12(?!\d)/iu},
  {composer:'Robert Schumann',title:'Ende vom Lied',match:/^ende vom lied[.!]?$/iu},
  {composer:'Gabriel Fauré',title:'Impromptu No. 3 in A-flat major, Op. 34',match:/impromptu|即[兴興]曲/iu,number:3,key:/a[-\s]?flat|[降♭]\s*a|a\s*[降♭]/iu},
  {composer:'Gabriel Fauré',title:'Nocturne No. 6 in D-flat major, Op. 63',match:/nocturne|夜曲/iu,number:6,key:/d[-\s]?flat|[降♭]\s*d|d\s*[降♭]/iu},
  {composer:'Gabriel Fauré',title:'Nocturne No. 13 in B minor, Op. 119',match:/nocturne|夜曲/iu,number:13,key:/b[-\s]+minor|b\s*小[调調]/iu},
  {composer:'Maurice Ravel',title:'Gaspard de la nuit',match:/^(?:gaspard de la nuit|夜[之的][幽游]灵|夜之幽靈)[.!。]?$/iu}
];
const numberPattern=/(?:\bn(?:o\.?|[°ºo])\s*|第\s*)(\d+|十三|三|六)(?![\d一二三四五六七八九十])/iu;
export const opusLine=/^(?:op(?:us)?\.?\s*|作品\s*)\d+[a-z]?(?:\s*(?:no\.?|之)\s*\d+)?[.,。]?$/iu;

export function composerCredit(text){
  for(const [composer,heading,prefix] of creditPattern){
    const match=text.match(prefix);
    if(match)return {composer,title:match[1].trim()};
    if(heading.test(text))return {composer,title:''};
  }
  return null;
}

export function verifiedProgramWork(title,credit,confirmedWorks){
  const text=title.normalize('NFKC').replace(/\bn[°º]\s*/giu,'no. ');
  const explicit=composerCredit(credit)?.composer||credit;
  const number=text.match(numberPattern)?.[1];
  const workNumber=({'三':3,'六':6,'十三':13})[number]||Number(number);
  const wrongMode=/\bminor\b|小[调調]/iu.test(text);
  const wrongMajor=/\bmajor\b|大[调調]/iu.test(text);
  return catalogue.find(work=>{
    if(explicit&&explicit!==work.composer)return false;
    if(!work.match.test(text)||work.id&&!work.id.test(text))return false;
    if(!work.number)return !/(?:\s[IVX]+\.\s*|\bmovement\s+\d|第[一二三四五六七八九十\d]+乐章)/iu.test(text);
    if(workNumber!==work.number||!work.key.test(text))return false;
    if(work.number===13?wrongMajor:wrongMode)return false;
    // Without a composer credit, only the full catalogue identity can confirm a numbered work.
    if(explicit||confirmedWorks?.has(work))return true;
    const entryOpus=work.title.match(/Op\. (\d+)/u)[1];
    return new RegExp('(?:\\bop\\.?|作品)\\s*'+entryOpus+'(?!\\d)','iu').test(text);
  });
}
