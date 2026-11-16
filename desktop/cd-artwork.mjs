import convert from 'heic-convert';

export async function convertCdArtwork(bytes){
  if(!(bytes instanceof Uint8Array)||!bytes.length||bytes.length>20*1024*1024)throw Error('HEIC 图片为空或超过 20 MB。');
  try{return new Uint8Array(await convert({buffer:Buffer.from(bytes),format:'PNG'}));}
  catch{throw Error('无法读取此 HEIC 图片。可将原图导出为 JPG 或 PNG 后重试。');}
}
