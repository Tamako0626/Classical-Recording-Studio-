import {readFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {openProjectMedia} from './project-media.mjs';
import {DatabaseSync} from 'node:sqlite';

export function openWorkspace(path){
  const sqlite=new DatabaseSync(path);
  let media;
  try{
    media=openProjectMedia(sqlite,join(dirname(path),'project-media','images'));
    sqlite.exec(readFileSync(new URL('../drizzle/0000_workspace.sql',import.meta.url),'utf8'));
  }catch(error){
    sqlite.close();
    throw error;
  }
  return {
    media,
    close:()=>sqlite.close(),
    DB:{prepare(sql){
      const statement=sqlite.prepare(sql);
      return {bind(...values){return {first:()=>statement.get(...values),run:()=>({meta:statement.run(...values)})};}};
    }}
  };
}
