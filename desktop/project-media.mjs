import {createHash,randomUUID} from 'node:crypto';
import {extname,join} from 'node:path';
import {mkdir,readFile,rename,rm,stat,writeFile} from 'node:fs/promises';
import {coverValid} from '../web/script.js';

export function openProjectMedia(db,folder){
  db.exec('CREATE TABLE IF NOT EXISTS studio_media (id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL, path TEXT NOT NULL, UNIQUE(kind,name,path))');
  const lookup=db.prepare('SELECT id,kind,name,path FROM studio_media WHERE id = ?');
  const insert=db.prepare('INSERT OR IGNORE INTO studio_media(id,kind,name,path) VALUES (?,?,?,?)');
  const identify=db.prepare('SELECT id FROM studio_media WHERE kind = ? AND name = ? AND path = ?');
  const get=id=>typeof id==='string'&&id.length<=80?lookup.get(id):undefined;
  function remember(kind,name,path){
    insert.run(randomUUID(),kind,name,path);
    return identify.get(kind,name,path).id;
  }
  async function saveImage(file){
    if(!coverValid(file?.name)||!(file.bytes instanceof Uint8Array)||!file.bytes.length||file.bytes.length>30*1024*1024)throw Error('无法保存封面或封底。请选择有效的 PNG 或 JPG。');
    const bytes=Buffer.from(file.bytes);
    const ext=extname(file.name).toLowerCase();
    const png=bytes.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]));
    const jpeg=bytes[0]===255&&bytes[1]===216&&bytes[2]===255;
    if(ext==='.png'?!png:!jpeg)throw Error('封面或封底的内容与扩展名不一致。');
    const hash=createHash('sha256').update(bytes).digest('hex');
    await mkdir(folder,{recursive:true});
    const path=join(folder,hash+(png?'.png':'.jpg'));
    const staged=path+'.'+randomUUID();
    try{await writeFile(staged,bytes,{flag:'wx'});await rename(staged,path);}
    finally{await rm(staged,{force:true});}
    return remember('image',file.name,path);
  }
  async function loadImage(id){
    const entry=get(id);
    if(entry?.kind!=='image')return null;
    try{
      const info=await stat(entry.path);
      if(!info.isFile()||!info.size||info.size>30*1024*1024)return null;
      return {name:entry.name,type:extname(entry.path)==='.png'?'image/png':'image/jpeg',bytes:await readFile(entry.path)};
    }catch{return null;}
  }
  return {get,remember,saveImage,loadImage};
}
