const groups=[
  ['降噪与修复',[
    ['denoise','降噪方式',['off','fft','nlm'],['关闭','FFT 底噪','非局部均值']],
    ['noiseReduction','FFT 降噪量（dB）',1,30,1,8],
    ['hum','电源嗡声',['off','50','60'],['关闭','50 Hz','60 Hz']],
    ['declick','去爆点'],
    ['declip','修复削波']
  ]],
  ['频率与声道',[
    ['highpass','高通（Hz，0 关闭）',0,250,1,0],
    ['lowpass','低通（Hz，0 关闭）',0,20000,1,0],
    ['bass','低频 100 Hz（dB）',-15,15,.5,0],
    ['mid','中频 1 kHz（dB）',-15,15,.5,0],
    ['treble','高频 6 kHz（dB）',-15,15,.5,0],
    ['channels','声道',['source','mono','swap','invert'],['保持来源','混为单声道','交换左右声道','反转右声道极性']]
  ]],
  ['动态与淡化',[
    ['gate','噪声门（dBFS，0 关闭）',-60,0,1,0],
    ['compress','压缩比（1 关闭）',1,8,.1,1],
    ['threshold','压缩阈值（dBFS）',-60,0,1,-18],
    ['deess','减弱齿音'],
    ['normalize','响度处理',['off','loud','dynamic'],['关闭','EBU R128 响度','动态归一化']],
    ['lufs','目标响度（LUFS）',-30,-10,1,-18],
    ['fadeIn','淡入（秒）',0,30,.05,0],
    ['fadeOut','淡出（秒）',0,30,.05,0]
  ]]
];
const limiterSetting=['limiter','峰值上限（dBFS）',-18,0,.1,0];
const controls=[...groups.flatMap(([,items])=>items),limiterSetting];
const defaultCeiling=-1;
export const effectDefaults=Object.fromEntries(controls.map(([key,,a,b,c,fallback])=>[key,Array.isArray(a)?a[0]:typeof a==='number'?fallback:false]));
const db=value=>+(10**(value/20)).toFixed(7);

export function effectsValid(values){
  if(values===undefined)return true;
  if(!values||typeof values!=='object'||Array.isArray(values)||Object.keys(values).some(key=>!Object.hasOwn(effectDefaults,key)))return false;
  const e={...effectDefaults,...values};
  return controls.every(([key,,a,b])=>Array.isArray(a)?a.includes(e[key]):typeof a==='number'?Number.isFinite(e[key])&&e[key]>=a&&e[key]<=b:typeof e[key]==='boolean')
    &&(!e.lowpass||e.lowpass>=100&&e.lowpass>e.highpass)&&(!e.gate||e.gate<=-1);
}
export function effectPlan(values){
  if(!effectsValid(values))throw Error('后期处理参数无效。低通至少为 100 Hz 且高于高通，噪声门阈值须为负数或 0（关闭）。');
  const e={...effectDefaults,...values};
  const before=[],after=[],tail=[];
  if(e.highpass)before.push('highpass=f='+e.highpass);
  if(e.lowpass)before.push('lowpass=f='+e.lowpass);
  if(e.hum!=='off'){
    for(let harmonic=1;harmonic<=3;harmonic++)before.push('equalizer=f='+Number(e.hum)*harmonic+':t=q:w=20:g=-24');
  }
  if(e.declip)before.push('adeclip');
  if(e.declick)before.push('adeclick');
  if(e.denoise==='fft')before.push('afftdn=nr='+e.noiseReduction+':tn=1');
  if(e.denoise==='nlm')before.push('anlmdn=s=0.0001');
  if(e.bass)before.push('bass=f=100:g='+e.bass);
  if(e.mid)before.push('equalizer=f=1000:t=q:w=1:g='+e.mid);
  if(e.treble)before.push('treble=f=6000:g='+e.treble);
  if(e.gate)before.push('agate=threshold='+db(e.gate)+':ratio=3:attack=10:release=250');
  if(e.channels==='swap')before.push('pan=stereo|c0=c1|c1=c0');
  if(e.channels==='invert')before.push('pan=stereo|c0=c0|c1=-1*c1');
  if(e.channels==='mono')after.push('aformat=channel_layouts=mono');
  if(e.deess)after.push('deesser');
  if(e.compress>1)after.push('acompressor=threshold='+db(e.threshold)+':ratio='+e.compress+':attack=20:release=250');
  if(e.normalize==='loud')after.push('loudnorm=I='+e.lufs+':TP=-1.5:LRA=11');
  if(e.normalize==='dynamic')after.push('dynaudnorm');
  if(e.limiter)tail.push('alimiter=limit='+db(e.limiter)+':level=false:latency=true');
  return {before,after,tail,fadeIn:e.fadeIn,fadeOut:e.fadeOut,stereo:['swap','invert'].includes(e.channels)};
}
export function fadeFilters({fadeIn,fadeOut},duration){
  if(fadeOut&&(!Number.isFinite(duration)||duration<=0))throw Error('淡出需要有效的音轨时长，请填写终点。');
  const filters=[];
  const seconds=value=>+value.toFixed(6);
  if(fadeIn)filters.push('afade=t=in:d='+seconds(Math.min(fadeIn,duration||Infinity)));
  if(fadeOut)filters.push('afade=t=out:st='+seconds(Math.max(0,duration-fadeOut))+':d='+seconds(Math.min(duration,fadeOut)));
  return filters;
}
export function effectsEnabled(values){
  const e=effectPlan(values);
  return !!(e.before.length||e.after.length||e.tail.length||e.fadeIn||e.fadeOut);
}
export function limiterForm(values){
  const [,label,min,max]=limiterSetting;
  const ceiling=effectsValid(values)&&values?.limiter||0;
  return '<div class="gain-limiter"><label class="gain-limiter-toggle"><input type="checkbox" name="effect-limiter-enabled"'+(ceiling?' checked':'')+'> 启用防爆音限幅</label>'+
    '<label class="field gain-limiter-ceiling"><span>'+label+'</span><input type="number" name="effect-limiter" min="'+min+'" max="'+max+'" step="any" value="'+(ceiling||defaultCeiling)+'"></label></div>';
}
export function effectsForm(values){
  const e=effectsValid(values)?{...effectDefaults,...values}:effectDefaults;
  return '<details class="audio-effects section-gap"'+(effectsEnabled({...e,limiter:0})?' open':'')+'><summary>高级后期处理</summary>'+
    groups.map(([title,items])=>'<fieldset><legend>'+title+'</legend><div class="effects-grid">'+items.map(([key,label,a,b,c])=>{
      const name=' name="effect-'+key+'"';
      if(Array.isArray(a))return '<label class="field"><span>'+label+'</span><select'+name+'>'+a.map((value,i)=>'<option value="'+value+'"'+(value===e[key]?' selected':'')+'>'+b[i]+'</option>').join('')+'</select></label>';
      if(typeof a==='number')return '<label class="field"><span>'+label+'</span><input type="number"'+name+' min="'+a+'" max="'+b+'" step="'+c+'" value="'+e[key]+'"></label>';
      return '<label><input type="checkbox"'+name+(e[key]?' checked':'')+'> '+label+'</label>';
    }).join('')+'</div></fieldset>').join('')+
    '<p class="hint">默认关闭。FFT 适用于持续底噪，非局部均值处理较慢。响度和动态处理建议试听后再批量导出。</p></details>';
}
export function readEffects(form,validate=true){
  const limitEnabled=form.querySelector('[name="effect-limiter-enabled"]').checked;
  const e=Object.fromEntries(Object.keys(effectDefaults).map(key=>{
    if(key==='limiter'&&!limitEnabled)return [key,0];
    const input=form.querySelector('[name="effect-'+key+'"]');
    const value=effectDefaults[key];
    return [key,typeof value==='boolean'?input.checked:typeof value==='number'?input.value.trim()?Number(input.value):NaN:input.value];
  }));
  if(limitEnabled&&(!Number.isFinite(e.limiter)||e.limiter<limiterSetting[2]||e.limiter>=0)){
    if(validate)throw Error('限幅峰值上限须小于 0 dBFS，最低为 −18 dBFS。');
    e.limiter=NaN;
  }
  if(validate&&!effectsValid(e))throw Error('请核对高级后期处理参数。');
  return e;
}
