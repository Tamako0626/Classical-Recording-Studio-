import assert from 'node:assert/strict';
import {mkdtemp,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {test} from 'node:test';
import {openWorkspace} from '../desktop/workspace.mjs';

test('reopening the workspace preserves existing projects',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'studio-workspace-test-'));
  let env;
  try{
    const path=join(dir,'workspace.sqlite');
    const payload=JSON.stringify({projects:[{name:'Existing recital'}],templates:[],logs:[]});
    env=openWorkspace(path);
    env.DB.prepare('INSERT INTO studio_workspace VALUES (?, ?, ?)').bind('desktop-user',1,payload).run();
    env.close();
    env=null;
    env=openWorkspace(path);
    const row=env.DB.prepare('SELECT revision,payload FROM studio_workspace WHERE user_id=?').bind('desktop-user').first();
    assert.equal(row.revision,1);
    assert.equal(row.payload,payload);
  }finally{
    env?.close();
    await rm(dir,{recursive:true,force:true});
  }
});
