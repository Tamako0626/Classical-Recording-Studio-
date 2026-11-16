import {test} from 'node:test';
import assert from 'node:assert/strict';
import {Buffer} from 'node:buffer';
import {decodeCue,parseCue,readCdText,sortCdFiles,cdFilename} from '../web/cd-import-data.js';

const cue=`REM DATE 1989
PERFORMER "Pianist"
TITLE "Piano Recital"
FILE "Disc.wav" WAVE
  TRACK 01 AUDIO
    TITLE "Allegro"
    INDEX 00 00:00:00
    INDEX 01 00:02:01
  TRACK 02 AUDIO
    TITLE "Adagio"
    INDEX 01 03:10:74
FILE "Disc Two.flac" WAVE
  TRACK 03 AUDIO
    TITLE "Finale"
    PERFORMER "Other Pianist"
    INDEX 01 00:00:00`;

test('CUE maps INDEX 01 to the matching source and respects each file end',()=>{
  const data=Buffer.concat([Buffer.from([255,254]),Buffer.from(cue,'utf16le')]);
  const files=[{name:'Disc.flac'},{name:'Disc Two.flac'}];
  const {album,tracks}=parseCue(decodeCue(data),files);
  assert.deepEqual(album,{date:'1989',artist:'Pianist',title:'Piano Recital'});
  assert.deepEqual(tracks.map(({source,start,end,title})=>[source,start,end,title]),[
    ['Disc.flac','00:00:02.013','00:03:10.987','Allegro'],
    ['Disc.flac','00:03:10.987','','Adagio'],
    ['Disc Two.flac','00:00:00.000','','Finale']
  ]);
  assert.equal(tracks[2].artist,'Other Pianist');
  assert.equal(cdFilename(tracks[0],0),'01 - Allegro');
  assert.throws(()=>parseCue(cue,files.slice(0,1)),/Disc Two/);
  assert.throws(()=>parseCue(cue.replace('03:10:74','03:10:75'),files),/时间无效/);
});
test('separate tracks use numeric filename order and real printed track numbers',()=>{
  const files=sortCdFiles(['Track10.wav','Track2.wav','Track1.wav'].map(name=>({name})));
  assert.deepEqual(files.map(file=>file.name),['Track1.wav','Track2.wav','Track10.wav']);
  const result=readCdText('Album: Sonata Cycle\nYear: 1992\n01 - Allegro 5:30\n03. Allegro 2:20\n03. Presto');
  assert.equal(result.album.title,'Sonata Cycle');
  assert.equal(result.album.date,'1992');
  assert.equal(result.numbered.get(1).title,'Allegro');
  assert.equal(result.numbered.get(3).title,'Allegro');
  assert.equal(result.numbered.has(2),false);
  assert.equal(result.issues.length,1);
});


test('CD artwork model labels carry album metadata separately from track metadata',()=>{
  const result=readCdText([
    '**Album Title:** "Historical Piano Recordings"','- **Album Artist:** Leo Sirota',
    '专辑作曲家：Frédéric Chopin','发行年份：１９６８',
    '**01.** Polonaise | Composer: Frédéric Chopin','02. Intermezzo | Composer: Robert Schumann'
  ].join('\n'));
  assert.deepEqual(result.album,{title:'Historical Piano Recordings',artist:'Leo Sirota',composer:'Frédéric Chopin',date:'1968'});
  assert.equal(result.numbered.get(2).composer,'Robert Schumann');
  assert.equal(readCdText('Album Date: 1982-11-15').album.date,'1982-11-15');
  assert.equal(readCdText('Album Artist: Unknown').album.artist,'');
});
