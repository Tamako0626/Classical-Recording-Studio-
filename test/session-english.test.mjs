import {test} from 'node:test';
import assert from 'node:assert/strict';
import {modelSessionReview} from '../web/session-english.js';

test('session suggestions preserve dates, omit new fields, and flag city and venue conflicts',()=>{
  const raw=JSON.stringify({fields:{date:{value:'2020-01-01'},city:{value:'Beijing'},
    venue:{value:'Shanghai Symphony Hall'},performer:{value:'Dénes Várjon'},composer:{value:'贝多芬'},
    orchestra:{value:'Shanghai Symphony Orchestra'}}});
  const output=modelSessionReview(raw,{date:'2026-11-06',city:'北京',venue:'上海交响音乐厅',performer:'瓦尔荣',composer:'贝多芬',orchestra:''});
  assert.deepEqual(output.map(item=>[item.key,item.after,!!item.needsReview]),[
    ['city','Beijing',true],['venue','Shanghai Symphony Hall',true],['performer','Dénes Várjon',false]
  ]);
});
