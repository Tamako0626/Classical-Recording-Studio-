import fs from 'node:fs/promises';
const assets = Object.fromEntries(await Promise.all(['index.html','style.css','app.js','script.js','seed.js','splitter.js','cover-studio.js','session-english.js','ffmpeg-worker.js','work-catalog.js','work-movements.js','program-parser.js','program-english.js','program-credits.js','program-bilingual.js','program-import.js','cd-import.js','cd-import-data.js','llm-files.js','llm-settings.js','audio-effects.js','artwork.js'].map(async name => ['/' + name, await fs.readFile('web/' + name, 'utf8')])));
const desktop=process.argv.includes('--desktop');
if(!desktop)assets['/ffmpeg-core.js'] = await fs.readFile('node_modules/@ffmpeg/core/dist/umd/ffmpeg-core.js','utf8');
for(const [route,file] of [
  ['/program-ocr.js','tesseract.js/dist/tesseract.esm.min.js'],
  ['/program-ocr-worker.js','tesseract.js/dist/worker.min.js'],
  ['/program-ocr-core.js','tesseract.js-core/tesseract-core-lstm.wasm.js']
])assets[route]=await fs.readFile('node_modules/'+file,'utf8');
if(desktop){
  for(const [route,file] of [
    ['/llm-pdf.mjs','pdfjs-dist/build/pdf.min.mjs'],
    ['/llm-pdf-worker.mjs','pdfjs-dist/build/pdf.worker.min.mjs'],
    ['/llm-zip.js','fflate/esm/browser.js']
  ])assets[route]=await fs.readFile('node_modules/'+file,'utf8');
  for(const [prefix,dir] of [['llm-cmaps','cmaps'],['llm-fonts','standard_fonts']]){
    for(const file of await fs.readdir('node_modules/pdfjs-dist/'+dir)){
      if(!file.startsWith('LICENSE'))assets['/'+prefix+'/'+file]=(await fs.readFile('node_modules/pdfjs-dist/'+dir+'/'+file)).toString('base64');
    }
  }
}
await fs.mkdir('dist/server', {recursive:true});
await fs.mkdir('dist/.openai', {recursive:true});
await fs.writeFile('dist/server/index.js', 'const assets = ' + JSON.stringify(assets) + ';\n' + await fs.readFile('server/work-search.js','utf8') + '\n' + await fs.readFile('server/cover-design.js','utf8') + '\n' + await fs.readFile('server/worker.js','utf8'));
if(!desktop)await fs.copyFile('.openai/hosting.json','dist/.openai/hosting.json');
