import {coverValid} from './script.js';

const baseTypes='.png,.jpg,.jpeg';
const heifTypes='.heic,.heif';
export const artworkTypes=converter=>baseTypes+(converter?','+heifTypes:'');
const heifImage=/\.(?:heic|heif)$/iu;
const maxInput=20*1024*1024;
const maxPng=30*1024*1024;

async function shrinkLargePng(blob){
  const image=await createImageBitmap(blob);
  try{
    const factor=Math.min(1,2560/Math.max(image.width,image.height));
    const canvas=document.createElement('canvas');
    canvas.width=Math.max(1,Math.round(image.width*factor));
    canvas.height=Math.max(1,Math.round(image.height*factor));
    canvas.getContext('2d').drawImage(image,0,0,canvas.width,canvas.height);
    return await new Promise(resolve=>canvas.toBlob(resolve,'image/png'));
  }finally{image.close();}
}

export async function prepareArtwork(file,convert,{webp=false,limit=maxInput}={}){
  if(!file)return null;
  const isHeif=heifImage.test(file.name);
  const targetName=isHeif?file.name.replace(heifImage,'.png'):file.name;
  const valid=coverValid(targetName)||(webp&&/\.webp$/iu.test(file.name));
  if(!valid||isHeif&&!convert||!file.size||file.size>(isHeif?maxInput:limit)){
    throw Error('请选择有效的 '+(convert?'HEIC、HEIF、':'')+'PNG 或 JPG'+(webp?'，也可选择 WebP':'')+' 图片，并核对文件大小。');
  }
  if(!isHeif)return file;
  const bytes=await convert(new Uint8Array(await file.arrayBuffer()));
  let blob=new Blob([bytes],{type:'image/png'});
  if(blob.size>maxPng)blob=await shrinkLargePng(blob);
  if(!blob?.size||blob.size>maxPng)throw Error('转换后的图片超过 30 MB，请缩小原图后重试。');
  return new File([blob],targetName,{type:'image/png'});
}
