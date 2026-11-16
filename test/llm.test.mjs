import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createLlm} from '../desktop/llm.mjs';

test('LLM connection, protected credentials, multimodal request and endpoint changes',async()=>{
  const folder=await mkdtemp(join(tmpdir(),'studio-llm-'));
  const key='mock-key';
  const storage={
    isEncryptionAvailable:()=>true,
    encryptString:()=>Buffer.from([1,2,3]),
    decryptString:bytes=>{assert.deepEqual(bytes,Buffer.from([1,2,3]));return key;}
  };
  const requests=[];
  const request=async(url,options)=>{
    requests.push({url,options,body:options.body&&JSON.parse(options.body)});
    assert.equal(options.redirect,'error');
    if(url.endsWith('/models'))return Response.json({data:[{id:'example-vision'},{id:'example-vision'}]});
    return Response.json({choices:[{message:{content:'1. Beethoven: Piano Sonata No. 6'},finish_reason:'stop'}]});
  };
  const llm=createLlm(folder,storage,request);
  const config={provider:'compatible',base:'http://127.0.0.1:8000/v1/',model:'example-vision',vision:true,key};
  try{
    assert.deepEqual(await llm.test(config),{models:['example-vision'],chat:true,chatError:''});
    assert.deepEqual(await llm.models(config),['example-vision']);
    const saved=await llm.save(config);
    assert.equal(saved.hasKey,true);
    assert.equal(saved.key,undefined);
    assert.doesNotMatch(await readFile(join(folder,'llm.json'),'utf8'),/mock-key/);
    const payload={task:'program',text:'abridged program',images:[Uint8Array.from([255,216,255,217])],
      endpoint:saved.base,model:saved.model,vision:saved.vision};
    assert.match(await llm.recognize(payload),/Beethoven/);
    const last=requests.at(-1);
    assert.equal(last.options.headers.Authorization,'Bearer '+key);
    assert.equal(last.body.messages[1].content[1].image_url.url,'data:image/jpeg;base64,/9j/2Q==');

    const other=await llm.save({...saved,base:'https://service.example/v1'});
    assert.equal(other.hasKey,false);
    await assert.rejects(llm.recognize(payload),/模型设置已变更/);
    await llm.test(other);
    assert.equal(requests.at(-1).options.headers.Authorization,undefined);
    await assert.rejects(llm.save({...saved,base:'http://service.example/v1'}),/HTTPS/);
  }finally{await rm(folder,{recursive:true,force:true});}
});

test('model movement suggestions preserve optional original numbers and reject duplicate rows',async()=>{
  const {modelMovementList,movementTitles}=await import('../web/work-movements.js');
  const candidate={title:'Symphonic Etudes, Op. 13',composer:'Robert Schumann',parts:[
    {number:'I.',name:'Theme. Andante'},{number:'Anh. I',name:'Variation I'}
  ]};
  const work=modelMovementList(candidate);
  assert.deepEqual(movementTitles(work.title,work.entries),[
    'Symphonic Etudes, Op. 13 – I. Theme. Andante','Symphonic Etudes, Op. 13 – Anh. I. Variation I'
  ]);
  assert.equal(modelMovementList({parts:[]}),null);
  assert.throws(()=>modelMovementList({...candidate,parts:[candidate.parts[0],candidate.parts[0]]}),/重复/);
});

test('session search sends only concert fields and returns candidates with matching web evidence',async()=>{
  const directory=await mkdtemp(join(tmpdir(),'studio-session-search-'));
  try{
    let request;
    const service=createLlm(directory,{isEncryptionAvailable:()=>true,encryptString:key=>Buffer.from(key),decryptString:bytes=>bytes.toString()},async(url,options)=>{
      request={url,body:JSON.parse(options.body)};
      return Response.json({choices:[{finish_reason:'stop',message:{
        content:JSON.stringify({candidates:[
          {session:{name:'Solo Recital',date:'2026-11-15',origin:'invented'},urls:['https://venue.example/recital#date','https://unknown.example/']},
          {session:{name:'Unverified recital',date:'2026-02-30'},urls:['https://unknown.example/']}
        ]}),
        server_tool_calls:[{calls:[{type:'deepseek_web_search',status:'completed',sources:[{url:'https://venue.example/recital',title:'Venue calendar'}]}]}]
      }}]});
    });
    await service.save({provider:'deepseek',base:'https://api.deepseek.com',model:'deepseek-flash',key:'test-session-key'});
    const result=await service.searchSession({performer:'Michel Dalberto',date:'2026-11-15',origin:'private',unknown:'local data'});
    assert.equal(request.url,'https://api.deepseek.com/chat/completions');
    assert.deepEqual(request.body.tools,[{type:'deepseek_web_search'}]);
    assert.deepEqual(JSON.parse(request.body.messages[1].content),{performer:'Michel Dalberto',date:'2026-11-15'});
    assert.deepEqual(result,[{session:{name:'Solo Recital',date:'2026-11-15'},sources:[{url:'https://venue.example/recital',title:'Venue calendar'}]}]);
    await service.save({provider:'compatible',base:'http://localhost:9000',model:'mock'});
    await assert.rejects(service.searchSession({performer:'Michel Dalberto'}),/DeepSeek/);
  }finally{await rm(directory,{recursive:true,force:true});}
});
