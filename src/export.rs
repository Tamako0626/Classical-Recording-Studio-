//! 分轨导出: 时间点, 标签, 封面和滤镜都交给裁剪版 ffmpeg.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::effects::filter_chain;
use crate::error::{StudioError, StudioResult};
use crate::ffmpeg::{self, Binaries, Cancel, MediaInfo};
use crate::model::{Project, Settings, Tags, TAG_KEYS};
use crate::names::{self, album_folder_name, filename_for, joined_end, output_filename_valid, track_tags};

#[derive(Clone, Debug)]
pub struct Job {
    pub order: usize,
    pub source: PathBuf,
    pub source_name: String,
    pub target: String,
    pub start: f64,
    pub end: Option<f64>,
    pub tags: Tags,
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
    let rule = if project.naming_rule.trim().is_empty() {
        names::DEFAULT_NAMING
    } else {
        project.naming_rule.trim()
    };
    let mut jobs = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    if settings.split {
        if project.tracks.is_empty() {
            return Err(StudioError::new("请至少添加一条曲目."));
        }
        for (index, track) in project.tracks.iter().enumerate() {
            let end_text = joined_end(&project.tracks, index, project.auto_join);
            let start = names::time_seconds(&track.start)?.unwrap_or(0.0);
            let end = names::time_seconds(&end_text)?;
            if let Some(end) = end {
                if end <= start {
                    return Err(StudioError::new(format!("第 {} 轨的结束须晚于开始.", index + 1)));
                }
            }
            let source = resolve_source(project, &track.source)?;
            let tags = track_tags(project, track, index);
            if settings.metadata && tags.title.trim().is_empty() {
                return Err(StudioError::new(format!("第 {} 轨缺少 Title.", index + 1)));
            }
            let base = filename_for(rule, track, &tags, index);
            let target = format!("{base}.{}", settings.format.to_lowercase());
            if base.is_empty() || !output_filename_valid(&target) {
                return Err(StudioError::new(format!("第 {} 轨的输出文件名无效或过长.", index + 1)));
            }
            if !seen.insert(target.to_lowercase()) {
                return Err(StudioError::new(format!("输出文件名重复: {target}")));
            }
            jobs.push(Job {
                order: index + 1,
                source,
                source_name: track.source.clone(),
                target,
                start,
                end,
                tags,
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
            let base = filename_for(rule, &dummy, &tags, index);
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
        let file = PathBuf::from(path);
        if file.is_file() {
            return Ok(file);
        }
    }
    let direct = PathBuf::from(name);
    if direct.is_file() {
        return Ok(direct);
    }
    Err(StudioError::new(format!("找不到录音 {name}. 请重新关联.")))
}

#[derive(Clone, Debug)]
pub struct TrackReport {
    pub target: String,
    pub path: Option<PathBuf>,
    pub error: Option<String>,
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
        match export_one(binaries, plan, job, &folder, cancel) {
            Ok(path) => reports.push(TrackReport {
                target: job.target.clone(),
                path: Some(path),
                error: None,
            }),
            Err(error) => {
                if cancel.is_cancelled() {
                    break;
                }
                reports.push(TrackReport {
                    target: job.target.clone(),
                    path: None,
                    error: Some(error.to_string()),
                });
            }
        }
    }
    Ok(reports)
}

fn export_one(binaries: &Binaries, plan: &Plan, job: &Job, folder: &Path, cancel: &Cancel) -> StudioResult<PathBuf> {
    let destination = folder.join(&job.target);
    if destination.exists() {
        return Err(StudioError::new(format!("输出目录已有同名文件: {}", job.target)));
    }
    let media = ffmpeg::probe(binaries, &job.source, cancel)?;
    if let Some(duration) = media.duration {
        if job.start >= duration || job.end.is_some_and(|end| end > duration + 0.05) {
            return Err(StudioError::new("起止点超出录音时长."));
        }
    }
    let span = job.end.map(|end| end - job.start).or_else(|| media.duration.map(|duration| duration - job.start));
    let filters = filter_chain(&plan.settings, span, media.channels)?;
    let scratch = tempfile_dir(folder)?;
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
        format!("{}", job.start),
        "-i".into(),
        job.source.display().to_string(),
    ];
    let image_input = plan.format != "MKV" && plan.settings.cover && plan.cover.is_some();
    if image_input {
        args.push("-i".into());
        args.push(plan.cover.as_ref().unwrap().display().to_string());
    }
    args.extend(["-map".into(), "0:a:0".into(), "-map_chapters".into(), "-1".into(), "-map_metadata".into(), "-1".into()]);
    if let Some(end) = job.end {
        args.push("-t".into());
        args.push(format!("{}", end - job.start));
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
    if image_input {
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
    if plan.format == "MKV" {
        if let Some(cover) = &plan.cover {
            if plan.settings.cover {
                let mime = if cover.extension().and_then(|ext| ext.to_str()).unwrap_or("").eq_ignore_ascii_case("png") {
                    "image/png"
                } else {
                    "image/jpeg"
                };
                let filename = cover.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| "cover.jpg".into());
                args.extend([
                    "-attach".into(),
                    cover.display().to_string(),
                    "-metadata:s:t:0".into(),
                    format!("mimetype={mime}"),
                    "-metadata:s:t:0".into(),
                    format!("filename={filename}"),
                ]);
            }
        }
    }
    args.push(staged.display().to_string());
    ffmpeg::run(&binaries.ffmpeg, &args, true, None, cancel)?;
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
    let _ = fs::remove_dir_all(&scratch);
    Ok(destination)
}

fn verify_output(info: &MediaInfo, plan: &Plan, job: &Job, span: Option<f64>) -> StudioResult<()> {
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
            if *key == "Track" {
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

fn archive_artwork(folder: &Path, source: Option<&Path>, back: bool) -> StudioResult<()> {
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
    let mut filters = Vec::new();
    for index in 0..paths.len() {
        filters.push(format!("[{index}:a:0]aformat=sample_fmts=s32:sample_rates={rate}:channel_layouts={layout},asetpts=N/SR/TB[a{index}]"));
    }
    let joined = (0..paths.len()).map(|index| format!("[a{index}]")).collect::<String>();
    filters.push(format!("{joined}concat=n={}:v=0:a=1[joined]", paths.len()));
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
