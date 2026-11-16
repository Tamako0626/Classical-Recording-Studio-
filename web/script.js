import {effectPlan,effectsValid} from './audio-effects.js';
import {tags} from './seed.js';

const audio = /\.(wav|flac|mp3|mkv|ape)$/i;
const cover = /\.(png|jpe?g)$/i;
const windowsLeaf = name => name && name !== '.' && name !== '..' && !/[<>:"/\\|?*\x00-\x1f]|[. ]$/u.test(name) && !/^(con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])(?:\.|$)/i.test(name);
const clean = value => value.replace(/[<>:"/\\|?*\x00-\x1f]/gu,' - ').replace(/\s+/gu,' ').trim().replace(/[. ]+$/u,'');
const formats = ['FLAC','WAV','MP3','MKV'];
export const sourceValid = name => windowsLeaf(name) && audio.test(name);
export const outputFilenameValid = name => !!windowsLeaf(name) && name.length <= 210;
export const coverValid = name => windowsLeaf(name) && cover.test(name);
export const outputValid = name => windowsLeaf(name) && name.length <= 100;
export function timeSeconds(value) {
  if (value === '') return null;
  if (!/^\d+(?::[0-5]\d){0,2}(?:\.\d{1,3})?$/u.test(value)) return NaN;
  return value.split(':').reduce((total,part)=>total*60+Number(part),0);
}
const trackNumber=index=>String(index+1).padStart(2,'0');
function sessionTagValues(project){
  const composer=/[/、;；]|\s[&＆+]\s/u.test(project.composer||'')?'':project.composer;
  return {Artist:project.performer||project.conductor,Album:project.name,Date:project.albumDate||project.date,Composer:composer,
    Conductor:project.conductor,Soloist:project.performer,Orchestra:project.orchestra};
}
function trackTags(project,track,index) {
  const values = {
    ...sessionTagValues(project),Track:trackNumber(index),
    ...Object.fromEntries(Object.entries(track.tags ?? {}).filter(([,v])=>v?.trim()))
  };
  return Object.fromEntries(tags.filter(key=>values[key]).map(key=>[key,values[key].trim()]));
}
export function batchSessionTags(project,tracks){
  const shared=sessionTagValues(project);
  shared.Artist ||= project.orchestra;
  const updates=new Map();
  const hasSession=Object.values(shared).some(value=>value?.trim());
  if(!hasSession)return {updates,matched:0,hasSession};
  let matched=0;
  for(const [index,track] of tracks.entries()){
    const previous=track.tags||{};
    if(!track.source&&!track.filename&&!track.end&&(!track.start||track.start==='00:00')&&!Object.values(previous).some(value=>value?.trim()))continue;
    matched++;
    const fields=Object.entries({...shared,Track:trackNumber(index)}).flatMap(([key,value])=>{
      const after=value?.trim();
      const before=previous[key];
      if(!after||after===before||['Track','Composer'].includes(key)&&before?.trim())return [];
      return [{key,before,after}];
    });
    if(fields.length)updates.set(track.id,fields);
  }
  return {updates,matched,hasSession};
}
export function filenameFor(rule,track,values,index) {
  const stem = track.source?.replace(audio,'') || '';
  return clean(track.filename?.trim().replace(audio,'') || rule.replace(/\{(\w+)\}/gu,(_,key)=>key==='Source'?stem:key==='Order'?String(index+1).padStart(2,'0'):values[key] || ''));
}
export function makePlan(project,settings,rule,output,selected) {
  const errors = [];
  if (!outputValid(output)) errors.push('输出目录须是当前目录下的单个 Windows 文件夹名。');
  if (!formats.includes(settings.format)) errors.push('请选择有效的输出格式。');
  if (![44100,48000,96000].includes(settings.rate)) errors.push('请选择支持的采样率。');
  if (settings.format==='MP3' && settings.rate===96000) errors.push('MP3 支持 44.1 或 48 kHz，请调整采样率。');
  for (const [key,min,max,label] of [['gain',-60,30,'增益'],['width',0,250,'声场拓宽'],['reverb',0,100,'混响']]) {
    if (!Number.isFinite(settings[key]) || settings[key] < min || settings[key] > max) errors.push(label+'参数超出范围。');
  }
  if (!effectsValid(settings.effects)) errors.push('高级后期处理参数无效。');
  if (settings.cover && project.coverFile && !coverValid(project.coverFile)) errors.push('封面须为当前目录中的 PNG 或 JPG 文件名。');
  const format = settings.cover && settings.format==='WAV' ? 'FLAC' : settings.format;
  const rows = settings.split
    ? project.tracks.filter(t=>!selected || selected.includes(t.id))
    : project.sources.filter(source=>!selected || selected.includes(source)).map(source=>({source,tags:project.tracks.find(t=>t.source===source && !t.end && (!t.start || timeSeconds(t.start)===0))?.tags ?? {}}));
  if (!rows.length) errors.push(settings.split?'请选择至少一条曲目。':'请登记并选择至少一个原始文件。');
  const seen = new Set();
  const jobs = rows.map(track=>{
    const order = settings.split ? project.tracks.indexOf(track) : project.sources.indexOf(track.source);
    const values = trackTags(project,track,order);
    const label = '第 '+(order+1)+' 项';
    if (track.awaitingCue && !track.start?.trim()) errors.push(label+'还没有起点，请填写时间。');
    const start = timeSeconds(track.start || '') ?? 0;
    const end = timeSeconds(track.end || '');
    if (!sourceValid(track.source) || !project.sources.includes(track.source)) errors.push(label+'需要选择已登记的原始文件。');
    if (!Number.isFinite(start) || (end!==null && (!Number.isFinite(end) || end<=start))) errors.push(label+'时间点无效，结束须晚于开始。');
    if (settings.metadata && settings.split && !values.Title) errors.push(label+'缺少 Title。');
    const base = filenameFor(rule,track,values,order);
    const target = base+'.'+format.toLowerCase();
    if (!base || !outputFilenameValid(target)) errors.push(label+'输出文件名无效或过长。');
    if (seen.has(target.toLowerCase())) errors.push('输出文件名重复：'+target);
    seen.add(target.toLowerCase());
    return {source:track.source,target,start,end,tags:settings.metadata?values:{},order:order+1};
  });
  return {errors,plan:{project:project.id,output,format,coverFile:settings.cover?project.coverFile:'',settings,jobs,effects:effectsValid(settings.effects)?effectPlan(settings.effects):null}};
}

const runtime = String.raw`
$ErrorActionPreference = 'Stop'
$plan = ConvertFrom-Json -InputObject ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('__PLAN__')))
$root = (Get-Location).ProviderPath
if (-not $root -or -not (Test-Path -LiteralPath $root -PathType Container)) { throw 'Open PowerShell in the recording folder.' }
$ffmpeg = (Get-Command ffmpeg -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
$ffprobe = (Get-Command ffprobe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
$out = Join-Path $root $plan.output
[void][IO.Directory]::CreateDirectory($out)
if ((Get-Item -LiteralPath $out).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'The output folder cannot be a junction or symbolic link.' }
$runId = [Guid]::NewGuid().ToString('N')
$log = Join-Path $out ('processing-' + $runId + '.jsonl')
$utf8 = New-Object Text.UTF8Encoding($false)
$culture = [Globalization.CultureInfo]::InvariantCulture

function Invoke-Native([string]$exe, [string[]]$argv) {
    $info = New-Object Diagnostics.ProcessStartInfo
    $info.FileName = $exe
    $info.Arguments = (($argv | ForEach-Object { '"' + ([regex]::Replace([regex]::Replace($_, '(\\*)"', '$1$1\"'), '(\\+)$', '$1$1')) + '"' }) -join ' ')
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.StandardOutputEncoding = [Text.Encoding]::UTF8
    $info.StandardErrorEncoding = [Text.Encoding]::UTF8
    $process = [Diagnostics.Process]::Start($info)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        $message = $stderr.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) { throw (([IO.Path]::GetFileName($exe)) + ' exit ' + $process.ExitCode + ': ' + $message) }
        return $stdout.GetAwaiter().GetResult()
    }
    finally { $process.Dispose() }
}

function Read-Media([string]$path) {
    Invoke-Native $ffprobe @('-v','error','-show_streams','-show_format','-of','json',$path) | ConvertFrom-Json
}

function Write-Result($job, [string]$status, [string]$message, $steps, $duration) {
    $entry = [ordered]@{schema=1; projectId=$plan.project; runId=$runId; time=[DateTimeOffset]::Now.ToString('o'); status=$status;
        source=$job.source; output=$job.target; message=$message; steps=@($steps); duration=$duration; settings=$plan.settings}
    [IO.File]::AppendAllText($log, (($entry | ConvertTo-Json -Compress -Depth 8) + [Environment]::NewLine), $utf8)
    Write-Host ('[' + $status + '] ' + $job.target + ' ' + $message)
}

function Resolve-Cover {
    if (-not $plan.settings.cover) { return $null }
    $names = @($plan.coverFile, 'cover.png', 'cover.jpg', 'cover.jpeg') | Where-Object { $_ } | Select-Object -Unique
    foreach ($name in $names) {
        $path = Join-Path $root $name
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
        $media = Read-Media $path
        $picture = $media.streams | Where-Object { $_.codec_type -eq 'video' } | Select-Object -First 1
        $extension = [IO.Path]::GetExtension($path).ToLowerInvariant()
        if ($picture.codec_name -eq 'png' -and $extension -eq '.png') { return @{Path=$path;Mime='image/png';Hash=(Get-FileHash -LiteralPath $path).Hash} }
        if ($picture.codec_name -eq 'mjpeg' -and $extension -in @('.jpg','.jpeg')) { return @{Path=$path;Mime='image/jpeg';Hash=(Get-FileHash -LiteralPath $path).Hash} }
        throw ('Cover extension or image format is invalid: ' + $name)
    }
    throw 'Cover not found. Use cover.png or cover.jpg, or enter the exact image filename.'
}

$picture = $null
$coverError = ''
try { $picture = Resolve-Cover } catch { $coverError = $_.Exception.Message }
$sourceHashes = @{}
$sourceInfo = @{}
$failed = 0
foreach ($job in $plan.jobs) {
    $temporary = $null
    $steps = @()
    $duration = $null
    try {
        if ($coverError) { throw $coverError }
        $source = Join-Path $root $job.source
        $target = Join-Path $out $job.target
        $receipt = $target + '.job-id'
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw ('Source not found: ' + $job.source) }
        if (-not $sourceHashes.ContainsKey($source)) {
            $sourceHashes[$source] = (Get-FileHash -LiteralPath $source).Hash
            $sourceInfo[$source] = Read-Media $source
        }
        $inputAudio = $sourceInfo[$source].streams | Where-Object { $_.codec_type -eq 'audio' } | Select-Object -First 1
        if (-not $inputAudio) { throw 'The source contains no audio stream.' }
        $durationText = $inputAudio.duration
        if (-not $durationText -or $durationText -eq 'N/A') { $durationText = $sourceInfo[$source].format.duration }
        $sourceLength = 0.0
        $knownLength = [double]::TryParse([string]$durationText,[Globalization.NumberStyles]::Float,$culture,[ref]$sourceLength)
        if ($knownLength -and ([double]$job.start -ge $sourceLength -or ($null -ne $job.end -and [double]$job.end -gt ($sourceLength + 0.25)))) { throw 'Split time exceeds source duration.' }
        $signatureText = ('studio-2026-11-13', $sourceHashes[$source],$picture.Hash,$plan.format,($plan.settings | ConvertTo-Json -Compress -Depth 6),($job | ConvertTo-Json -Compress -Depth 6)) -join '|'
        $sha = [Security.Cryptography.SHA256]::Create()
        try { $signature = [BitConverter]::ToString($sha.ComputeHash($utf8.GetBytes($signatureText))) } finally { $sha.Dispose() }
        if (Test-Path -LiteralPath $target) {
            if ((Test-Path -LiteralPath $receipt -PathType Leaf) -and (Test-Path -LiteralPath $target -PathType Leaf) -and (Get-Item -LiteralPath $target).Length -gt 0) {
                $previous = [IO.File]::ReadAllText($receipt).Split('|')
                if ($previous.Count -eq 2 -and $previous[0] -eq $signature -and $previous[1] -eq (Get-FileHash -LiteralPath $target).Hash) {
                    Write-Result $job 'skipped' 'Identical verified output already exists.' @() $null
                    continue
                }
            }
            throw 'Output already exists with a different or missing receipt. Choose another output folder or review the existing file.'
        }
        if (Test-Path -LiteralPath $receipt) { throw 'An output receipt already exists. Review the folder or choose another output folder.' }
        $temporary = Join-Path $out ('.processing-' + [Guid]::NewGuid().ToString('N') + [IO.Path]::GetExtension($target))
        $ff = @('-nostdin','-hide_banner','-v','error','-xerror','-n')
        if ($job.start -gt 0) { $ff += @('-ss',([double]$job.start).ToString('0.###',$culture)) }
        $ff += @('-i',$source)
        if ($picture -and $plan.format -ne 'MKV') { $ff += @('-i',$picture.Path) }
        $ff += @('-map','0:a:0','-map_chapters','-1')
        if ($null -ne $job.end) { $ff += @('-t',([double]($job.end-$job.start)).ToString('0.###',$culture)) }
        if ($plan.settings.split) { $steps += 'split' }
        if ([IO.Path]::GetFileName($source) -cne $job.target) { $steps += 'rename' }
        if ([IO.Path]::GetExtension($source) -ine [IO.Path]::GetExtension($target) -or [int]$inputAudio.sample_rate -ne $plan.settings.rate) { $steps += 'format' }
        if ($plan.settings.metadata) {
            $ff += @('-map_metadata','-1')
            foreach ($tag in $job.tags.PSObject.Properties) { $ff += @('-metadata',($tag.Name + '=' + [string]$tag.Value)) }
            $steps += 'tags'
        }
        $filters = @()
        if ($plan.effects.stereo -and [int]$inputAudio.channels -ne 2) { throw 'Channel swap and polarity inversion require stereo audio.' }
        if ($plan.effects.before) { $filters += @($plan.effects.before) }
        if ($plan.settings.gain -ne 0) {
            $filters += ('volume=' + ([double]$plan.settings.gain).ToString($culture) + 'dB')
            $steps += 'gain'
        }
        if ($plan.settings.width -ne 100) {
            if ([int]$inputAudio.channels -ne 2) { throw 'Stereo widening requires a two-channel source.' }
            $filters += ('extrastereo=c=0:m=' + ([double]($plan.settings.width/100)).ToString($culture))
            $steps += 'width'
        }
        if ($plan.settings.reverb -gt 0) {
            $mix = [double]($plan.settings.reverb/100)
            $filters += ('aecho=1:1:35|61|89:' + (($mix*0.22).ToString($culture),($mix*0.14).ToString($culture),($mix*0.08).ToString($culture) -join '|'))
            $steps += 'reverb'
        }
        if ($plan.effects.after) { $filters += @($plan.effects.after) }
        if ($plan.effects.fadeIn -gt 0 -or $plan.effects.fadeOut -gt 0) {
            $span = [double]::PositiveInfinity
            if ($knownLength) { $span = $sourceLength - [double]$job.start }
            if ($null -ne $job.end) { $span = [double]$job.end - [double]$job.start }
            if ($plan.effects.fadeIn -gt 0) {
                $fadeIn = [Math]::Min([double]$plan.effects.fadeIn,$span)
                $filters += ('afade=t=in:d=' + $fadeIn.ToString('0.######',$culture))
            }
            if ($plan.effects.fadeOut -gt 0) {
                if ([double]::IsInfinity($span) -or $span -le 0) { throw 'Fade-out requires a known track length.' }
                $fadeOut = [Math]::Min([double]$plan.effects.fadeOut,$span)
                $filters += ('afade=t=out:st=' + ($span-$fadeOut).ToString('0.######',$culture) + ':d=' + $fadeOut.ToString('0.######',$culture))
            }
        }
        if ($plan.effects.tail) { $filters += @($plan.effects.tail) }
        if ($plan.effects.before -or $plan.effects.after -or $plan.effects.tail -or $plan.effects.fadeIn -gt 0 -or $plan.effects.fadeOut -gt 0) { $steps += 'effects' }
        if ($filters.Count) { $ff += @('-af',($filters -join ',')) }
        $ff += @('-ar',[string]$plan.settings.rate)
        switch ($plan.format) {
            'FLAC' { $ff += @('-c:a','flac','-sample_fmt','s32','-bits_per_raw_sample','24') }
            'WAV' { $ff += @('-c:a','pcm_s24le') }
            'MP3' { $ff += @('-c:a','libmp3lame','-q:a','2','-id3v2_version','3') }
            'MKV' { $ff += @('-c:a','pcm_s24le') }
        }
        if ($picture -and $plan.format -ne 'MKV') {
            $ff += @('-map','1:v:0','-c:v','copy','-disposition:v:0','attached_pic','-metadata:s:v:0','title=Album cover','-metadata:s:v:0','comment=Cover (front)')
        }
        if ($picture -and $plan.format -eq 'MKV') {
            $ff += @('-attach',$picture.Path,'-metadata:s:t:0',('mimetype=' + $picture.Mime),'-metadata:s:t:0',('filename=' + [IO.Path]::GetFileName($picture.Path)))
        }
        if ($picture) { $steps += 'cover' }
        [void](Invoke-Native $ffmpeg ($ff + @($temporary)))
        $media = Read-Media $temporary
        $encoded = $media.streams | Where-Object { $_.codec_type -eq 'audio' } | Select-Object -First 1
        $duration = [double]::Parse([string]$media.format.duration,$culture)
        if ($duration -le 0 -or [int]$encoded.sample_rate -ne $plan.settings.rate) { throw 'Quality check failed: invalid duration or sample rate.' }
        if ($null -ne $job.end -and [Math]::Abs($duration-($job.end-$job.start)) -gt 0.30) { throw 'Quality check failed: unexpected split duration.' }
        if ($picture -and -not ($media.streams | Where-Object { $_.disposition.attached_pic -eq 1 -or $_.codec_type -eq 'attachment' })) { throw 'Quality check failed: cover is missing.' }
        [void](Invoke-Native $ffmpeg @('-nostdin','-v','error','-xerror','-i',$temporary,'-map','0:a:0','-f','null','-'))
        $steps += 'check'
        $targetHash = (Get-FileHash -LiteralPath $temporary).Hash
        [IO.File]::Move($temporary,$target)
        $handle = [IO.File]::Open($receipt,[IO.FileMode]::CreateNew)
        try {
            $receiptBytes = $utf8.GetBytes($signature + '|' + $targetHash)
            $handle.Write($receiptBytes,0,$receiptBytes.Length)
        } finally { $handle.Dispose() }
        Write-Result $job 'success' '' $steps $duration
    }
    catch {
        $failed++
        Write-Result $job 'failed' $_.Exception.Message $steps $duration
    }
    finally {
        if ($temporary -and [IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) }
    }
}
Write-Host ('Log: ' + $log)
if ($failed) { Write-Warning ('Failed items: ' + $failed) }
`;
export function renderScript(plan) {
  const bytes = new TextEncoder().encode(JSON.stringify(plan));
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return "# Classical Recording Studio\r\n# Run from the folder containing your original recordings.\r\n& {\r\n"+runtime.trim().replace('__PLAN__',btoa(binary)).replace(/\n/g,'\r\n')+"\r\n}\r\n";
}
