//! 分轨导出: 时间点, 标签, 封面和滤镜都交给裁剪版 ffmpeg.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::effects::filter_chain;
use crate::error::{StudioError, StudioResult};
use crate::ffmpeg::{self, Binaries, Cancel, MediaInfo};
use crate::model::{Project, Segment, Settings, Tags, TAG_KEYS};
use crate::names::{self, album_folder_name, filename_for, joined_end, output_filename_valid, track_tags};

#[derive(Clone, Debug)]
pub struct SpanCut {
    pub source: PathBuf,
    pub start: f64,
    pub end: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct Job {
    pub order: usize,
    pub source: PathBuf,
    pub source_name: String,
    pub target: String,
    pub start: f64,
    pub end: Option<f64>,
    pub tags: Tags,
    pub disc: u32,
    pub segments: Vec<SpanCut>,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub folder_name: String,
    pub format: String,
    pub settings: Settings,
    pub jobs: Vec<Job>,
    pub cover: Option<PathBuf>,
    pub back_cover: Option<PathBuf>,
    pub note: String,
}

pub fn prepare(project: &Project) -> StudioResult<Plan> {
    if project.name.trim().is_empty() {
        return Err(StudioError::new("请填写场次 / 专辑名."));
    }
    let mut settings = project.settings.clone();
    let mut note = String::new();
    if settings.cover && settings.format == "WAV" {
        settings.format = "FLAC".into();
        note = "WAV 不能嵌入封面, 已改用 FLAC.".into();
    }
    if !matches!(settings.format.as_str(), "FLAC" | "WAV" | "MP3" | "MKV") {
        return Err(StudioError::new("请选择 FLAC, WAV, MP3 或 MKV."));
    }
    if !matches!(settings.rate, 44100 | 48000 | 96000) {
        return Err(StudioError::new("请选择 44100, 48000 或 96000 Hz."));
    }
    if settings.format == "MP3" && settings.rate == 96000 {
        return Err(StudioError::new("MP3 支持 44.1 或 48 kHz."));
    }
    let cover = optional_cover(&project.cover_path, &project.cover_file, settings.cover && settings.format != "MKV")?;
    let back_cover = optional_cover(&project.back_cover_path, &project.back_cover_file, false)?;
    let rule = names::effective_naming(project);
    let mut jobs = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    if settings.split {
        if project.tracks.is_empty() {
            return Err(StudioError::new("请至少添加一条曲目."));
        }
        for (index, track) in project.tracks.iter().enumerate() {
            let segments = cuts_for(project, track, index)?;
            let tags = track_tags(project, track, index);
            if settings.metadata && tags.title.trim().is_empty() {
                return Err(StudioError::new(format!("第 {} 轨缺少 Title.", index + 1)));
            }
            let base = filename_for(&rule, track, &tags, index);
            let leaf = format!("{base}.{}", settings.format.to_lowercase());
            if base.is_empty() || !output_filename_valid(&leaf) {
                return Err(StudioError::new(format!("第 {} 轨的输出文件名无效或过长.", index + 1)));
            }
            let target = if project.disc_folders && track.disc > 0 {
                format!("Disc {:02}/{leaf}", track.disc)
            } else {
                leaf
            };
            if !seen.insert(target.to_lowercase()) {
                return Err(StudioError::new(format!("输出文件名重复: {target}")));
            }
            let first = segments.first().cloned();
            jobs.push(Job {
                order: index + 1,
                source: first.as_ref().map(|cut| cut.source.clone()).unwrap_or_default(),
                source_name: track.source.clone(),
                target,
                start: first.as_ref().map(|cut| cut.start).unwrap_or(0.0),
                end: first.as_ref().and_then(|cut| cut.end),
                tags,
                disc: track.disc,
                segments,
            });
        }
    } else {
        if project.sources.is_empty() {
            return Err(StudioError::new("请先导入录音."));
        }
        for (index, source_name) in project.sources.iter().enumerate() {
            let source = resolve_source(project, source_name)?;
            let track = project.tracks.iter().find(|track| track.source == *source_name);
            let tags = track.map(|track| track_tags(project, track, index)).unwrap_or_default();
            let dummy = track.cloned().unwrap_or_else(|| {
                let mut blank = crate::model::Track::blank();
                blank.source = source_name.clone();
                blank
            });
            let base = filename_for(&rule, &dummy, &tags, index);
            let target = format!("{base}.{}", settings.format.to_lowercase());
            if base.is_empty() || !output_filename_valid(&target) {
                return Err(StudioError::new(format!("第 {} 个文件的输出文件名无效.", index + 1)));
            }
            if !seen.insert(target.to_lowercase()) {
                return Err(StudioError::new(format!("输出文件名重复: {target}")));
            }
            jobs.push(Job {
                order: index + 1,
                source,
                source_name: source_name.clone(),
                target,
                start: 0.0,
                end: None,
                tags,
                disc: 0,
                segments: Vec::new(),
            });
        }
    }
    Ok(Plan {
        folder_name: album_folder_name(project)?,
        format: settings.format.clone(),
        settings,
        jobs,
        cover,
        back_cover,
        note,
    })
}

fn cuts_for(project: &Project, track: &crate::model::Track, index: usize) -> StudioResult<Vec<SpanCut>> {
    let raw = if track.parts.is_empty() {
        vec![Segment {
            source: track.source.clone(),
            start: track.start.clone(),
            end: joined_end(&project.tracks, index, project.auto_join),
            title: String::new(),
        }]
    } else {
        track.parts.clone()
    };
    if raw.is_empty() {
        return Err(StudioError::new(format!("第 {} 轨没有片段.", index + 1)));
    }
    let mut cuts = Vec::new();
    for (part_index, part) in raw.iter().enumerate() {
        let source = resolve_source(project, &part.source)?;
        let start = names::time_seconds(&part.start)?.unwrap_or(0.0);
        let end = names::time_seconds(&part.end)?;
        if let Some(end) = end {
            if end <= start {
                return Err(StudioError::new(format!("第 {} 轨的结束须晚于开始.", index + 1)));
            }
        } else if part_index + 1 != raw.len() {
            return Err(StudioError::new(format!("第 {} 轨的中间片段缺少终点.", index + 1)));
        }
        cuts.push(SpanCut { source, start, end });
    }
    Ok(collapse_cuts(cuts))
}

fn collapse_cuts(cuts: Vec<SpanCut>) -> Vec<SpanCut> {
    let mut out = Vec::new();
    for cut in cuts {
        let join = out.last().is_some_and(|last: &SpanCut| last.source == cut.source && last.end.is_some_and(|end| (end - cut.start).abs() <= 0.05));
        if join {
            out.last_mut().unwrap().end = cut.end;
        } else {
            out.push(cut);
        }
    }
    out
}

fn optional_cover(path: &str, name: &str, required_decode: bool) -> StudioResult<Option<PathBuf>> {
    if path.trim().is_empty() {
        return Ok(None);
    }
    let file = PathBuf::from(path);
    if !file.is_file() {
        return Err(StudioError::new(format!("找不到封面: {path}")));
    }
    let leaf = if name.trim().is_empty() {
        file.file_name().map(|item| item.to_string_lossy().to_string()).unwrap_or_default()
    } else {
        name.to_string()
    };
    if !names::cover_name(&leaf) && required_decode {
        return Err(StudioError::new("封面须为 PNG 或 JPG."));
    }
    if file.metadata().map(|meta| meta.len()).unwrap_or(0) > 30 * 1024 * 1024 {
        return Err(StudioError::new("封面不能超过 30 MB."));
    }
    Ok(Some(file))
}

pub fn resolve_source(project: &Project, name: &str) -> StudioResult<PathBuf> {
    if name.trim().is_empty() {
        return Err(StudioError::new("有曲目还没有选择录音."));
    }
    if let Some(path) = project.source_paths.get(name) {
        if let Some(file) = existing_source(project, path) {
            return Ok(file);
        }
    }
    let direct = PathBuf::from(name);
    if direct.is_file() {
        return Ok(direct);
    }
    Err(StudioError::new(format!("找不到录音 {name}. 请重新关联.")))
}

fn existing_source(project: &Project, path: &str) -> Option<PathBuf> {
    let file = PathBuf::from(path);
    if file.is_file() {
        return Some(file);
    }
    if project.source_root.trim().is_empty() {
        return None;
    }
    let joined = PathBuf::from(&project.source_root).join(file);
    joined.is_file().then_some(joined)
}

pub fn describe_plan(plan: &Plan) -> String {
    let concat = plan.jobs.iter().filter(|job| job.segments.len() > 1).count();
    let mut lines = vec![format!("将导出 {} 个文件到 {}.", plan.jobs.len(), plan.folder_name)];
    if concat > 0 {
        lines.push(format!("{concat} 条由多段拼接, 段与段之间的空隙不会保留."));
    }
    for job in plan.jobs.iter().take(12) {
        let kind = if job.segments.len() > 1 {
            format!("拼接 {} 段", job.segments.len())
        } else {
            "单段".into()
        };
        lines.push(format!("{}  {kind}", job.target));
    }
    if plan.jobs.len() > 12 {
        lines.push(format!("其余 {} 个文件略.", plan.jobs.len() - 12));
    }
    lines.join("\n")
}

pub fn relink_directory(project: &mut Project, root: &Path) -> StudioResult<usize> {
    if !root.is_dir() {
        return Err(StudioError::new("请选择一个已有的文件夹."));
    }
    let mut by_leaf: std::collections::BTreeMap<String, Vec<PathBuf>> = std::collections::BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().map(|item| item.to_string_lossy().to_string()) else {
                continue;
            };
            if names::audio_name(&name) {
                by_leaf.entry(name.to_lowercase()).or_default().push(path);
            }
        }
    }
    let mut linked = 0usize;
    for source in project.sources.clone() {
        let leaf = names::source_leaf(&source).to_lowercase();
        let Some(paths) = by_leaf.get(&leaf) else {
            continue;
        };
        if paths.len() != 1 {
            continue;
        }
        let text = stored_path(project, &paths[0]);
        project.source_paths.insert(source, text);
        linked += 1;
    }
    Ok(linked)
}

fn stored_path(project: &Project, path: &Path) -> String {
    if !project.source_root.trim().is_empty() {
        if let Ok(relative) = path.strip_prefix(&project.source_root) {
            return relative.to_string_lossy().replace('\\', "/");
        }
    }
    path.display().to_string()
}

#[derive(Clone, Debug)]
pub struct TrackReport {
    pub target: String,
    pub path: Option<PathBuf>,
    pub error: Option<String>,
    pub skipped: bool,
}

pub fn export_album(
    binaries: &Binaries,
    plan: &Plan,
    parent: &Path,
    cancel: &Cancel,
    progress: &async_channel::Sender<String>,
) -> StudioResult<Vec<TrackReport>> {
    if !parent.is_dir() {
        return Err(StudioError::new("请选择一个已有的文件夹."));
    }
    let folder = parent.join(&plan.folder_name);
    fs::create_dir_all(&folder)?;
    if plan.cover.is_some() || plan.back_cover.is_some() {
        archive_artwork(&folder, plan.cover.as_deref(), false)?;
        archive_artwork(&folder, plan.back_cover.as_deref(), true)?;
    }
    let mut reports = Vec::new();
    for (index, job) in plan.jobs.iter().enumerate() {
        if cancel.is_cancelled() {
            break;
        }
        let _ = progress.try_send(format!("{}/{}  {}", index + 1, plan.jobs.len(), job.target));
        match export_one(binaries, plan, job, &folder, cancel, progress, index, plan.jobs.len()) {
            Ok(made) => reports.push(TrackReport {
                target: job.target.clone(),
                path: Some(made.path),
                error: None,
                skipped: made.skipped,
            }),
            Err(error) => {
                if cancel.is_cancelled() {
                    break;
                }
                reports.push(TrackReport {
                    target: job.target.clone(),
                    path: None,
                    error: Some(error.to_string()),
                    skipped: false,
                });
            }
        }
    }
    Ok(reports)
}

struct Made {
    path: PathBuf,
    skipped: bool,
}

fn export_one(
    binaries: &Binaries,
    plan: &Plan,
    job: &Job,
    folder: &Path,
    cancel: &Cancel,
    progress: &async_channel::Sender<String>,
    index: usize,
    total: usize,
) -> StudioResult<Made> {
    let destination = folder.join(&job.target);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let cuts = job_cuts(job);
    let span = span_of(binaries, &cuts, cancel)?;
    if destination.exists() {
        let existing = ffmpeg::probe(binaries, &destination, cancel)?;
        return if verify_output(&existing, plan, job, span).is_ok() {
            Ok(Made { path: destination, skipped: true })
        } else {
            Err(StudioError::new(format!("输出目录已有同名文件, 且与本次导出不一致: {}", job.target)))
        };
    }
    let scratch = tempfile_dir(folder)?;
    let _clean = CleanDir(scratch.clone());
    let (input, start, end, channels) = stage_cuts(binaries, &cuts, folder, cancel)?;
    let filters = filter_chain(&plan.settings, span, channels)?;
    let staged = scratch.join(format!("track{}", extension(&job.target)));
    let mut args = vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-y".into(),
        "-v".into(),
        "error".into(),
        "-xerror".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        "-ss".into(),
        format!("{start}"),
        "-i".into(),
        input.display().to_string(),
    ];
    let front = plan.format != "MKV" && plan.settings.cover && plan.cover.is_some();
    let back = plan.format != "MKV" && plan.settings.embed_back && plan.back_cover.is_some();
    if front {
        args.push("-i".into());
        args.push(plan.cover.as_ref().unwrap().display().to_string());
    }
    if back {
        args.push("-i".into());
        args.push(plan.back_cover.as_ref().unwrap().display().to_string());
    }
    args.extend(["-map".into(), "0:a:0".into(), "-map_chapters".into(), "-1".into(), "-map_metadata".into(), "-1".into()]);
    if let Some(end) = end {
        args.push("-t".into());
        args.push(format!("{}", end - start));
    }
    if plan.settings.metadata {
        for key in TAG_KEYS {
            let value = job.tags.get(key).trim();
            if !value.is_empty() && !value.contains('\0') {
                args.push("-metadata".into());
                args.push(format!("{key}={}", value.chars().take(1000).collect::<String>()));
            }
        }
    }
    if !filters.is_empty() {
        args.push("-af".into());
        args.push(filters.join(","));
    }
    args.push("-ar".into());
    args.push(plan.settings.rate.to_string());
    args.extend(encoder_args(&plan.format));
    if front {
        args.extend([
            "-map".into(),
            "1:v:0".into(),
            "-c:v".into(),
            "copy".into(),
            "-disposition:v:0".into(),
            "attached_pic".into(),
            "-metadata:s:v:0".into(),
            "title=Album cover".into(),
            "-metadata:s:v:0".into(),
            "comment=Cover (front)".into(),
        ]);
    }
    if back {
        let stream = if front { "2:v:0" } else { "1:v:0" };
        let index = if front { 1 } else { 0 };
        args.extend([
            "-map".into(),
            stream.into(),
            "-c:v".into(),
            "copy".into(),
            format!("-disposition:v:{index}"),
            "attached_pic".into(),
            format!("-metadata:s:v:{index}"),
            "title=Album cover".into(),
            format!("-metadata:s:v:{index}"),
            "comment=Cover (back)".into(),
        ]);
    }
    if plan.format == "MKV" {
        attach_mkv(&mut args, plan.cover.as_deref(), plan.settings.cover, 0);
        let back_index = usize::from(plan.settings.cover && plan.cover.is_some());
        attach_mkv(&mut args, plan.back_cover.as_deref(), plan.settings.embed_back, back_index);
    }
    args.push(staged.display().to_string());
    let label = format!("{}/{total}  {}", index + 1, job.target);
    let (sender, watcher) = watch_progress(progress, &label, span);
    let encoded = ffmpeg::run(&binaries.ffmpeg, &args, true, sender, cancel);
    if let Some(watcher) = watcher {
        let _ = watcher.join();
    }
    encoded?;
    let checked = ffmpeg::probe(binaries, &staged, cancel)?;
    verify_output(&checked, plan, job, span)?;
    ffmpeg::run(
        &binaries.ffmpeg,
        &[
            "-nostdin".into(),
            "-v".into(),
            "error".into(),
            "-xerror".into(),
            "-i".into(),
            staged.display().to_string(),
            "-map".into(),
            "0:a:0".into(),
            "-f".into(),
            "null".into(),
            "-".into(),
        ],
        false,
        None,
        cancel,
    )?;
    publish_new(&staged, &destination)?;
    Ok(Made { path: destination, skipped: false })
}

fn job_cuts(job: &Job) -> Vec<SpanCut> {
    if job.segments.is_empty() {
        vec![SpanCut { source: job.source.clone(), start: job.start, end: job.end }]
    } else {
        job.segments.clone()
    }
}

fn span_of(binaries: &Binaries, cuts: &[SpanCut], cancel: &Cancel) -> StudioResult<Option<f64>> {
    if cuts.is_empty() {
        return Ok(None);
    }
    let mut total = 0.0;
    let mut known = true;
    for cut in cuts {
        if let Some(end) = cut.end {
            if end <= cut.start {
                return Err(StudioError::new("起止点超出录音时长."));
            }
            total += end - cut.start;
            continue;
        }
        let media = ffmpeg::probe(binaries, &cut.source, cancel)?;
        if let Some(duration) = media.duration {
            if cut.start >= duration {
                return Err(StudioError::new("起止点超出录音时长."));
            }
            total += duration - cut.start;
        } else {
            known = false;
        }
    }
    Ok(known.then_some(total))
}

fn stage_cuts(binaries: &Binaries, cuts: &[SpanCut], folder: &Path, cancel: &Cancel) -> StudioResult<(PathBuf, f64, Option<f64>, u32)> {
    if cuts.len() <= 1 {
        let cut = cuts.first().ok_or_else(|| StudioError::new("没有可导出的片段."))?;
        let media = ffmpeg::probe(binaries, &cut.source, cancel)?;
        if let Some(duration) = media.duration {
            if cut.start >= duration || cut.end.is_some_and(|end| end > duration + 0.05) {
                return Err(StudioError::new("起止点超出录音时长."));
            }
        }
        return Ok((cut.source.clone(), cut.start, cut.end, media.channels));
    }
    let scratch = tempfile_dir(folder)?;
    let mut parts = Vec::new();
    let mut channels = 0u32;
    for (index, cut) in cuts.iter().enumerate() {
        let media = ffmpeg::probe(binaries, &cut.source, cancel)?;
        channels = channels.max(media.channels);
        if let Some(duration) = media.duration {
            if cut.start >= duration || cut.end.is_some_and(|end| end > duration + 0.05) {
                return Err(StudioError::new("起止点超出录音时长."));
            }
        }
        let part = scratch.join(format!("part-{index}.flac"));
        let mut args = vec![
            "-hide_banner".into(),
            "-nostdin".into(),
            "-y".into(),
            "-v".into(),
            "error".into(),
            "-xerror".into(),
            "-ss".into(),
            format!("{}", cut.start),
            "-i".into(),
            cut.source.display().to_string(),
            "-map".into(),
            "0:a:0".into(),
        ];
        if let Some(end) = cut.end {
            args.push("-t".into());
            args.push(format!("{}", end - cut.start));
        }
        args.extend(["-c:a".into(), "flac".into(), "-sample_fmt".into(), "s32".into(), part.display().to_string()]);
        ffmpeg::run(&binaries.ffmpeg, &args, false, None, cancel)?;
        parts.push(part);
    }
    let joined = scratch.join("joined.flac");
    concatenate(binaries, &parts, &joined, cancel)?;
    let media = ffmpeg::probe(binaries, &joined, cancel)?;
    Ok((joined, 0.0, None, media.channels.max(channels)))
}

fn watch_progress(progress: &async_channel::Sender<String>, label: &str, span: Option<f64>) -> (Option<async_channel::Sender<f64>>, Option<std::thread::JoinHandle<()>>) {
    let Some(span) = span.filter(|value| *value > 0.2) else {
        return (None, None);
    };
    let (sender, receiver) = async_channel::unbounded::<f64>();
    let progress = progress.clone();
    let label = label.to_string();
    let handle = std::thread::spawn(move || {
        while let Ok(seconds) = receiver.recv_blocking() {
            let percent = (seconds / span * 100.0).clamp(0.0_f64, 99.0_f64);
            let _ = progress.try_send(format!("{label}  {percent:.0}%"));
        }
    });
    (Some(sender), Some(handle))
}

fn attach_mkv(args: &mut Vec<String>, cover: Option<&Path>, enabled: bool, index: usize) {
    let Some(cover) = cover else {
        return;
    };
    if !enabled {
        return;
    }
    let mime = if cover.extension().and_then(|ext| ext.to_str()).unwrap_or("").eq_ignore_ascii_case("png") {
        "image/png"
    } else {
        "image/jpeg"
    };
    let filename = cover.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| "cover.jpg".into());
    args.extend([
        "-attach".into(),
        cover.display().to_string(),
        format!("-metadata:s:t:{index}"),
        format!("mimetype={mime}"),
        format!("-metadata:s:t:{index}"),
        format!("filename={filename}"),
    ]);
}

pub(crate) fn verify_output(info: &MediaInfo, plan: &Plan, job: &Job, span: Option<f64>) -> StudioResult<()> {
    let Some(duration) = info.duration else {
        return Err(StudioError::new("质量检查失败: 没有时长."));
    };
    if info.rate != plan.settings.rate {
        return Err(StudioError::new("质量检查失败: 采样率异常."));
    }
    if let Some(span) = span {
        if (duration - span).abs() > 0.3 {
            return Err(StudioError::new("质量检查失败: 时长异常."));
        }
    }
    if plan.settings.cover && plan.cover.is_some() && !info.attached_pic {
        return Err(StudioError::new("质量检查失败: 封面缺失."));
    }
    if plan.settings.metadata {
        let keys: &[&str] = if plan.format == "WAV" {
            &["Title", "Artist", "Album", "Track", "Date"]
        } else {
            &TAG_KEYS
        };
        for key in keys {
            let expected = job.tags.get(key).trim();
            if expected.is_empty() {
                continue;
            }
            let actual = info.tags.get(&key.to_ascii_lowercase()).map(String::as_str).unwrap_or("");
            if expected == actual {
                continue;
            }
            if matches!(*key, "Disc" | "TotalDiscs") && !matches!(plan.format.as_str(), "FLAC" | "MKV") {
                continue;
            }
            if matches!(*key, "Track" | "Disc" | "TotalDiscs") {
                if let (Ok(left), Ok(right)) = (expected.parse::<u32>(), actual.parse::<u32>()) {
                    if left == right {
                        continue;
                    }
                }
            }
            return Err(StudioError::new(format!("质量检查失败: {key} 标签未正确写入.")));
        }
    }
    Ok(())
}

fn encoder_args(format: &str) -> Vec<String> {
    match format {
        "FLAC" => ["-c:a", "flac", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"].map(str::to_string).to_vec(),
        "WAV" => ["-c:a", "pcm_s24le", "-rf64", "auto"].map(str::to_string).to_vec(),
        "MP3" => ["-c:a", "libmp3lame", "-q:a", "2", "-id3v2_version", "3"].map(str::to_string).to_vec(),
        _ => ["-c:a", "pcm_s24le"].map(str::to_string).to_vec(),
    }
}

fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_else(|| ".flac".into())
}

fn tempfile_dir(folder: &Path) -> StudioResult<PathBuf> {
    let dir = folder.join(format!(".studio-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

struct CleanDir(PathBuf);

impl Drop for CleanDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn publish_new(source: &Path, destination: &Path) -> StudioResult<()> {
    if destination.exists() {
        return Err(StudioError::new("输出目录已有同名文件."));
    }
    if fs::hard_link(source, destination).is_err() {
        let mut input = fs::File::open(source)?;
        let mut output = OpenOptions::new().write(true).create_new(true).open(destination)?;
        std::io::copy(&mut input, &mut output)?;
        output.flush()?;
    }
    Ok(())
}

pub(crate) fn archive_artwork(folder: &Path, source: Option<&Path>, back: bool) -> StudioResult<()> {
    let Some(source) = source else {
        return Ok(());
    };
    let ext = if source.extension().and_then(|ext| ext.to_str()).unwrap_or("").eq_ignore_ascii_case("png") {
        "png"
    } else {
        "jpg"
    };
    let name = if back { format!("back.{ext}") } else { format!("cover.{ext}") };
    let destination = folder.join(name);
    let bytes = fs::read(source)?;
    if destination.is_file() {
        let existing = fs::read(&destination)?;
        if existing != bytes {
            return Err(StudioError::new(format!("专辑文件夹已有不同的 {}.", destination.file_name().unwrap_or_default().to_string_lossy())));
        }
        return Ok(());
    }
    fs::write(destination, bytes)?;
    Ok(())
}

fn concat_filters(count: usize, rate: u32, layout: &str) -> Vec<String> {
    let mut filters = Vec::new();
    for index in 0..count {
        filters.push(format!("[{index}:a:0]aformat=sample_fmts=s32:sample_rates={rate}:channel_layouts={layout},asetpts=N/SR/TB[a{index}]"));
    }
    let joined = (0..count).map(|index| format!("[a{index}]")).collect::<String>();
    filters.push(format!("{joined}concat=n={count}:v=0:a=1[joined]"));
    filters
}

pub fn concatenate(binaries: &Binaries, paths: &[PathBuf], output: &Path, cancel: &Cancel) -> StudioResult<MediaInfo> {
    if paths.len() < 2 {
        return Err(StudioError::new("拼接至少需要两段录音."));
    }
    let mut infos = Vec::new();
    for path in paths {
        infos.push(ffmpeg::probe(binaries, path, cancel)?);
    }
    let rate = infos.iter().map(|info| info.rate).max().unwrap_or(48000);
    let channels = infos.iter().map(|info| info.channels).max().unwrap_or(2);
    let layout = if channels == 1 {
        "mono".to_string()
    } else if channels == 2 {
        "stereo".to_string()
    } else {
        infos[0].layout.clone()
    };
    if channels > 2 && infos.iter().any(|info| info.channels != channels || info.layout != layout) {
        return Err(StudioError::new("多声道拼接要求所有片段使用相同声道布局."));
    }
    let filters = concat_filters(paths.len(), rate, &layout);
    let mut args = vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-y".into(),
        "-v".into(),
        "error".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
    ];
    for path in paths {
        args.push("-i".into());
        args.push(path.display().to_string());
    }
    args.extend([
        "-filter_complex".into(),
        filters.join(";"),
        "-map".into(),
        "[joined]".into(),
        "-map_metadata".into(),
        "-1".into(),
        "-c:a".into(),
        "flac".into(),
        "-sample_fmt".into(),
        "s32".into(),
        output.display().to_string(),
    ]);
    ffmpeg::run(&binaries.ffmpeg, &args, true, None, cancel)?;
    ffmpeg::probe(binaries, output, cancel)
}

pub fn preview_span(
    binaries: &Binaries,
    source: &Path,
    start: f64,
    seconds: f64,
    filters: &[String],
    rate: u32,
    cancel: &Cancel,
) -> StudioResult<PathBuf> {
    let dir = crate::store::temp_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("preview-{}-{}.wav", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|item| item.as_millis()).unwrap_or(0)));
    let mut args = vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-y".into(),
        "-v".into(),
        "error".into(),
        "-ss".into(),
        format!("{start}"),
        "-i".into(),
        source.display().to_string(),
        "-map".into(),
        "0:a:0".into(),
        "-t".into(),
        format!("{seconds}"),
        "-ar".into(),
        rate.to_string(),
        "-c:a".into(),
        "pcm_s16le".into(),
    ];
    if !filters.is_empty() {
        args.push("-af".into());
        args.push(filters.join(","));
    }
    args.push(path.display().to_string());
    ffmpeg::run(&binaries.ffmpeg, &args, true, None, cancel)?;
    Ok(path)
}

pub fn powershell(plan: &Plan) -> String {
    let jobs = plan
        .jobs
        .iter()
        .map(|job| {
            let mut metadata = serde_json::Map::new();
            for key in TAG_KEYS {
                let value = job.tags.get(key).trim();
                if !value.is_empty() {
                    metadata.insert(key.into(), Value::String(value.chars().take(300).collect()));
                }
            }
            json!({
                "source": job.source.display().to_string(),
                "start": job.start,
                "end": job.end,
                "target": job.target,
                "metadata": metadata,
            })
        })
        .collect::<Vec<_>>();
    let body = json!({ "output": plan.folder_name, "format": plan.format, "jobs": jobs });
    let encoded = base64_encode(body.to_string().as_bytes());
    format!(
        r#"# Classical Recording Studio
# Run from any folder. Sources use the paths saved when this script was written.
$ErrorActionPreference = 'Stop'
$plan = ConvertFrom-Json -InputObject ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded}')))
$ffmpeg = (Get-Command ffmpeg -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
$out = Join-Path (Get-Location) $plan.output
[void][IO.Directory]::CreateDirectory($out)
$log = Join-Path $out ('processing-' + [Guid]::NewGuid().ToString('N') + '.jsonl')
$utf8 = New-Object Text.UTF8Encoding($false)
function Write-Result($job, [string]$status, [string]$message) {{
  $entry = [ordered]@{{ schema = 1; projectId = ''; time = [DateTimeOffset]::Now.ToString('o'); status = $status; source = $job.source; output = $job.target; message = $message }}
  [IO.File]::AppendAllText($log, (($entry | ConvertTo-Json -Compress) + [Environment]::NewLine), $utf8)
  Write-Host ('[' + $status + '] ' + $job.target + ' ' + $message)
}}
$failed = 0
foreach ($job in $plan.jobs) {{
  $target = Join-Path $out $job.target
  $parent = Split-Path -Parent $target
  if ($parent) {{ [void][IO.Directory]::CreateDirectory($parent) }}
  try {{
    $ff = @('-hide_banner','-nostdin','-y','-v','error','-ss',[string]$job.start,'-i',[string]$job.source,'-map','0:a:0')
    if ($null -ne $job.end) {{ $ff += @('-t', [string]([double]$job.end - [double]$job.start)) }}
    foreach ($pair in $job.metadata.PSObject.Properties) {{ $ff += @('-metadata', ($pair.Name + '=' + $pair.Value)) }}
    switch ($plan.format) {{
      'MP3' {{ $ff += @('-c:a','libmp3lame','-q:a','2') }}
      'WAV' {{ $ff += @('-c:a','pcm_s16le') }}
      'MKV' {{ $ff += @('-c:a','flac') }}
      default {{ $ff += @('-c:a','flac') }}
    }}
    $ff += $target
    & $ffmpeg @ff
    if ($LASTEXITCODE -ne 0) {{ throw ('ffmpeg exit ' + $LASTEXITCODE) }}
    Write-Result $job 'success' ''
  }} catch {{
    $failed++
    Write-Result $job 'failed' $_.Exception.Message
  }}
}}
Write-Host ('Log: ' + $log)
if ($failed) {{ Write-Warning ('Failed items: ' + $failed) }}
"#
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessingRow {
    pub project_id: String,
    pub time: String,
    pub status: String,
    pub source: String,
    pub output: String,
    pub message: String,
}

pub fn parse_processing_log(text: &str) -> Vec<ProcessingRow> {
    text.lines()
        .filter_map(|line| {
            let value: Value = serde_json::from_str(line.trim()).ok()?;
            let output = value.get("output").and_then(Value::as_str).unwrap_or("").trim().to_string();
            if output.is_empty() {
                return None;
            }
            Some(ProcessingRow {
                project_id: value.get("projectId").and_then(Value::as_str).unwrap_or("").to_string(),
                time: value.get("time").and_then(Value::as_str).unwrap_or("").chars().take(25).collect(),
                status: value.get("status").and_then(Value::as_str).unwrap_or("").to_string(),
                source: value.get("source").and_then(Value::as_str).unwrap_or("").to_string(),
                output,
                message: value.get("message").and_then(Value::as_str).unwrap_or("").chars().take(500).collect(),
            })
        })
        .collect()
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let value = ((bytes[index] as u32) << 16) | ((bytes[index + 1] as u32) << 8) | bytes[index + 2] as u32;
        for shift in [18, 12, 6, 0] {
            out.push(TABLE[((value >> shift) & 63) as usize] as char);
        }
        index += 3;
    }
    let rest = bytes.len() - index;
    if rest == 1 {
        let value = (bytes[index] as u32) << 16;
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let value = ((bytes[index] as u32) << 16) | ((bytes[index + 1] as u32) << 8);
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        out.push(TABLE[((value >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod script_tests {
    use super::*;
    use crate::model::Project;

    #[test]
    fn powershell_records_a_jsonl_log_and_the_plan_round_trips() {
        let file = std::env::temp_dir().join(format!("crs-script-{}.wav", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        let mut project = Project::blank();
        project.name = "Recital".into();
        project.sources.push("concert.wav".into());
        project.source_paths.insert("concert.wav".into(), file.display().to_string());
        project.tracks[0].source = "concert.wav".into();
        project.tracks[0].tags.title = "Sonata".into();
        project.tracks[0].tags.composer = "Beethoven".into();
        project.settings.metadata = false;
        project.settings.cover = false;
        let plan = prepare(&project).unwrap();
        let _ = std::fs::remove_file(&file);
        let script = powershell(&plan);
        assert!(script.contains("processing-"));
        assert!(script.contains("ffmpeg"));
        let encoded = script.split("FromBase64String('").nth(1).unwrap().split('\'').next().unwrap();
        assert!(!encoded.is_empty());
        let rows = parse_processing_log(r#"{"schema":1,"projectId":"p1","time":"2024-05-02T10:00:00Z","status":"success","source":"a.wav","output":"01.flac","message":""}"#);
        assert_eq!(rows[0].output, "01.flac");
        assert_eq!(rows[0].status, "success");
        assert!(parse_processing_log("not json").is_empty());
    }
}
