import {test} from 'node:test';
import assert from 'node:assert/strict';
import {batchSessionTags,makePlan} from '../web/script.js';
import {blankTrack,defaults} from '../web/seed.js';

test('batch session edits override shared tags and preserve track-specific metadata through export',()=>{
  const tracks=[
    {...blankTrack(),id:'first',source:'Original.wav',start:'00:00',end:'01:00'},
    {...blankTrack(),id:'second',source:'Original.wav',start:'01:00',end:'02:00'}
  ];
  Object.assign(tracks[0].tags,{Album:'Obsolete',Artist:'Previous',Title:'Sonata',Composer:'Beethoven',Track:'09'});
  Object.assign(tracks[1].tags,{Title:'Prélude',Date:'2001-04-03'});
  const project={id:'album',name:'New Recital',date:'2026-11-13',performer:'Michel Dalberto',conductor:'Example Conductor',
    composer:'Beethoven / Debussy',orchestra:'Example Orchestra',sources:['Original.wav'],tracks};
  const {updates,matched}=batchSessionTags(project,[...tracks,{...blankTrack(),start:'00:00'}]);
  assert.equal(matched,2);
  assert.equal(updates.size,2);
  for(const track of tracks)for(const {key,after} of updates.get(track.id))track.tags[key]=after;
  assert.equal(tracks[0].tags.Track,'09');
  assert.equal(tracks[0].tags.Composer,'Beethoven');
  assert.equal(tracks[1].tags.Composer,'');
  assert.equal(tracks[1].tags.Track,'02');
  const {plan,errors}=makePlan(project,{...defaults,cover:false},'{Track} - {Title}','Output');
  assert.deepEqual(errors,[]);
  assert.deepEqual(plan.jobs[0].tags,{Title:'Sonata',Artist:'Michel Dalberto',Album:'New Recital',Track:'09',Date:'2026-11-13',
    Composer:'Beethoven',Conductor:'Example Conductor',Soloist:'Michel Dalberto',Orchestra:'Example Orchestra'});
  assert.equal(plan.jobs[1].tags.Date,'2026-11-13');
  assert.equal(plan.jobs[1].tags.Composer,undefined);
  assert.equal(batchSessionTags({},tracks).hasSession,false);
  assert.ok(batchSessionTags({composer:'Debussy'},tracks).updates.get('second').some(field=>field.key==='Composer'&&field.after==='Debussy'));
});
