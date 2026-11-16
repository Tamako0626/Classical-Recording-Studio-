export const statuses = ['待整理','待核对时间点','已分轨','已重命名','已嵌入封面','已增益','已完成','处理失败','需修复'];
export const operations = ['分轨','重命名','重写标签','嵌入封面','增益调整','声场处理','混响处理','高级后期处理','格式转换','质量检查','错误记录','修订记录','脚本生成','创建项目'];
export const tags = ['Title','Artist','Album','Track','Date','Composer','Conductor','Soloist','Orchestra'];
export const defaults = {format:'FLAC',rate:48000,gain:0,width:100,reverb:0,cover:true,split:true,metadata:true};
export const naming = '{Track} - {Title}';
export const blankTrack = () => ({id:crypto.randomUUID(),source:'',start:'',end:'',filename:'',tags:Object.fromEntries(tags.map(key=>[key,'']))});
export const blankProject = () => ({id:crypto.randomUUID(),name:'',performer:'',conductor:'',composer:'',date:'',city:'',venue:'',origin:'',status:'待整理',coverFile:'',coverStatus:'待准备',sources:[],tracks:[],notes:'',output:'Processed',settings:{...defaults}});
function sample(data, rows) {
  const p = {...blankProject(),...data};
  p.tracks = rows.map(([source,start,end,Title,Composer],i)=>({
    ...blankTrack(),source,start,end,tags:{Title,Artist:p.performer || p.conductor,Album:p.name,Track:String(i+1).padStart(2,'0'),Date:p.date,Composer,Conductor:p.conductor,Soloist:p.performer,Orchestra:p.orchestra || ''}
  }));
  return p;
}
export function seed() {
  return {
    naming,
    logs:[],
    templates:[
      {id:'archive-flac',name:'分轨与专辑归档',rule:naming,output:'Processed',settings:{...defaults}},
      {id:'flac-cover',name:'整轨转 FLAC 并嵌入封面',rule:'{Source}',output:'FLAC Cover',settings:{...defaults,split:false}},
      {id:'gain-width',name:'增益与声场处理',rule:'{Source}',output:'Audio Processing',settings:{...defaults,split:false,cover:false,metadata:false,gain:16,width:130}}
    ],
    projects:[
      sample({id:'dalberto',name:'Michel Dalberto · Mozart Recital',performer:'Michel Dalberto',composer:'Wolfgang Amadeus Mozart / Claude Debussy',date:'2026-11-06',city:'上海',venue:'上海交响音乐厅',origin:'现场自录',status:'待核对时间点',coverFile:'cover.png',coverStatus:'待核对',sources:['Recording 1.wav','Recording 2.wav','Recording 3.wav'],notes:'参考既有整理信息。原始文件名使用占位名称，请替换为实际名称。若导入已增益的 FLAC，请先核对累计处理参数。'},[
        ['Recording 1.wav','00:34','12:40','Fantasia in C minor, K. 475','Wolfgang Amadeus Mozart'],
        ['Recording 1.wav','13:15','29:45','Piano Sonata in A minor, K. 310','Wolfgang Amadeus Mozart'],
        ['Recording 1.wav','30:07','','Piano Sonata No. 11 in A major, K. 331','Wolfgang Amadeus Mozart'],
        ['Recording 2.wav','01:15','10:39','Rondo in A minor, K. 511','Wolfgang Amadeus Mozart'],
        ['Recording 2.wav','10:47','22:37','Variations in D major, K. 573','Wolfgang Amadeus Mozart'],
        ['Recording 2.wav','22:57','','Piano Sonata in C minor, K. 457','Wolfgang Amadeus Mozart'],
        ['Recording 3.wav','00:00','','Clair de lune','Claude Debussy']
      ]),
      sample({id:'varjon',name:'Dénes Várjon · Solo Recital',performer:'Dénes Várjon',composer:'Johann Sebastian Bach / Ludwig van Beethoven / Robert Schumann',date:'2026-10-29',city:'上海',venue:'上海交响音乐厅',origin:'现场自录',coverFile:'cover.png',coverStatus:'待核对',notes:'曲目顺序已预填。请逐轨选择原始文件。'},[
        ['','','','English Suite No. 2 in A minor, BWV 807','Johann Sebastian Bach'],['','','','Piano Sonata No. 30 in E major, Op. 109','Ludwig van Beethoven'],['','','','Kinderszenen, Op. 15','Robert Schumann'],['','','','Fantasie in C major, Op. 17','Robert Schumann'],['','','','Encore','']
      ]),
      sample({id:'alexeev',name:'Dmitri Alexeev · Solo Recital',performer:'Dmitri Alexeev',composer:'Franz Liszt / Franz Schubert / Richard Wagner / Frédéric Chopin / Robert Schumann / Felix Mendelssohn',date:'2026-11-01',city:'上海',venue:'凯迪拉克·上海音乐厅',origin:'现场自录',status:'待核对时间点',sources:['261101194844.wav','261101201513.wav'],notes:'返场文件名和时间点待补录。'},[
        ['261101194844.wav','00:00','05:15','Ständchen','Franz Schubert / Franz Liszt'],['261101194844.wav','05:15','08:55','Aufenthalt','Franz Schubert / Franz Liszt'],['261101194844.wav','08:55','14:55','Der Wanderer','Franz Schubert / Franz Liszt'],['261101194844.wav','14:55','18:40','Litanei','Franz Schubert / Franz Liszt'],['261101194844.wav','18:40','','Isoldes Liebestod','Richard Wagner / Franz Liszt'],['261101201513.wav','00:00','06:02','Nocturne in A-flat major, Op. 32 No. 2','Frédéric Chopin'],['261101201513.wav','06:02','','Symphonic Etudes, Op. 13','Robert Schumann'],['','','','Song Without Words, Op. 62 No. 1','Felix Mendelssohn'],['','','','Frühlingsnacht','Robert Schumann / Franz Liszt'],['','','','Song Without Words, Op. 67 No. 2','Felix Mendelssohn']
      ]),
      sample({id:'zender',name:'Hans Zender · Tchaikovsky Symphony No. 6',conductor:'Hans Zender',composer:'Pyotr Ilyich Tchaikovsky',orchestra:'Hamburger Symphoniker',date:'1985-10-21',city:'汉堡',venue:'',origin:'历史现场录音',status:'需修复',notes:'此前末乐章曾出现 0 字节文件。请填写实际原始 MP3 文件名后再生成脚本。'},[
        ['','00:00','18:22','Symphony No. 6, Op. 74 - I. Adagio – Allegro non troppo','Pyotr Ilyich Tchaikovsky'],['','18:22','26:23','Symphony No. 6, Op. 74 - II. Allegro con grazia','Pyotr Ilyich Tchaikovsky'],['','26:23','36:08','Symphony No. 6, Op. 74 - III. Allegro molto vivace','Pyotr Ilyich Tchaikovsky'],['','36:08','','Symphony No. 6, Op. 74 - IV. Finale','Pyotr Ilyich Tchaikovsky']
      ])
    ]
  };
}
