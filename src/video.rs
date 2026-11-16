//! 静态封面视频. 画面用裁剪版 ffmpeg 的 libx264, 不解码别人的 H.264.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use crate::error::{StudioError, StudioResult};
use crate::ffmpeg::{self, Binaries, Cancel, MediaInfo};
use crate::names::{cover_name, output_filename_valid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoMode {
    Auto,
    Mp4,
    Mkv48,
}

struct AudioPlan {
    ext: &'static str,
    codec: String,
    rate: u32,
    args: Vec<String>,
    min_bits: u32,
}

pub fn make_cover_videos(
    binaries: &Binaries,
    sources: &[PathBuf],
    cover: &Path,
    folder: &Path,
    mode: VideoMode,
    cancel: &Cancel,
    progress: &async_channel::Sender<String>,
) -> StudioResult<Vec<(String, StudioResult<PathBuf>)>> {
    if sources.is_empty() || sources.len() > 500 {
        return Err(StudioError::new("请选择 1 至 500 条音轨."));
    }
    let cover_leaf = cover.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
    if !cover_name(&cover_leaf) {
        return Err(StudioError::new("封面须为 PNG 或 JPG."));
    }
    if cover.metadata().map(|meta| meta.len()).unwrap_or(0) > 30 * 1024 * 1024 {
        return Err(StudioError::new("封面不能超过 30 MB."));
    }
    let out_dir = folder.join("封面视频");
    fs::create_dir_all(&out_dir)?;
    let scratch = out_dir.join(format!(".studio-video-{}", std::process::id()));
    fs::create_dir_all(&scratch)?;
    let template = scratch.join("still.mkv");
    let still_args = vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-y".into(),
        "-v".into(),
        "error".into(),
        "-loop".into(),
        "1".into(),
        "-framerate".into(),
        "24".into(),
        "-i".into(),
        cover.display().to_string(),
        "-an".into(),
        "-vf".into(),
        "scale=1920:1080:force_original_aspect_ratio=decrease:force_divisible_by=2,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:black,setsar=1,format=yuv420p".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "ultrafast".into(),
        "-tune".into(),
        "stillimage".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-t".into(),
        "12".into(),
        template.display().to_string(),
    ];
    ffmpeg::run(&binaries.ffmpeg, &still_args, true, None, cancel)?;
    let mut results = Vec::new();
    let mut used = std::collections::BTreeSet::new();
    for (index, source) in sources.iter().enumerate() {
        if cancel.is_cancelled() {
            break;
        }
        let leaf = source.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| format!("track-{index}"));
        let _ = progress.try_send(format!("{}/{}  {leaf}", index + 1, sources.len()));
        let outcome = (|| -> StudioResult<PathBuf> {
            let info = ffmpeg::probe(binaries, source, cancel)?;
            let audio_len = info.duration.ok_or_else(|| StudioError::new("无法读取源音频长度."))?;
            let plan = audio_plan(&info, mode)?;
            let stem = source
                .file_stem()
                .map(|name| name.to_string_lossy().chars().take(190).collect::<String>())
                .unwrap_or_else(|| "video".into());
            let stem = crate::names::sanitize_leaf(&stem, 180);
            let mut target = format!("{stem}.{}", plan.ext);
            let mut count = 2;
            while used.contains(&target.to_lowercase()) {
                target = format!("{stem} ({count}).{}", plan.ext);
                count += 1;
            }
            if !output_filename_valid(&target) {
                return Err(StudioError::new("输出文件名无效."));
            }
            used.insert(target.to_lowercase());
            let destination = out_dir.join(&target);
            if destination.exists() {
                return Err(StudioError::new("输出目录已有同名文件."));
            }
            let staged = scratch.join(format!("video-{index}.{}", plan.ext));
            let mut args = vec![
                "-hide_banner".into(),
                "-nostdin".into(),
                "-y".into(),
                "-v".into(),
                "error".into(),
                "-stream_loop".into(),
                "-1".into(),
                "-i".into(),
                template.display().to_string(),
                "-i".into(),
                source.display().to_string(),
                "-map".into(),
                "0:v:0".into(),
                "-map".into(),
                "1:a:0".into(),
                "-map_metadata".into(),
                "1".into(),
                "-map_chapters".into(),
                "-1".into(),
                "-c:v".into(),
                "copy".into(),
                "-shortest".into(),
            ];
            args.extend(plan.args.clone());
            args.push(staged.display().to_string());
            ffmpeg::run(&binaries.ffmpeg, &args, true, None, cancel)?;
            let output = ffmpeg::probe(binaries, &staged, cancel)?;
            let duration = output.duration.ok_or_else(|| StudioError::new("成品没有时长."))?;
            if (duration - audio_len).abs() > 0.25
                || output.codec != plan.codec
                || output.rate != plan.rate
                || output.video_codec != "h264"
                || output.width != 1920
                || output.height != 1080
            {
                return Err(StudioError::new("成品的画面, 音频格式或时长未通过检查."));
            }
            if plan.ext == "mkv" && output.bits < plan.min_bits && plan.codec != "flac" {
                return Err(StudioError::new("成品位深未通过检查."));
            }
            if fs::hard_link(&staged, &destination).is_err() {
                let mut input = fs::File::open(&staged)?;
                let mut file = OpenOptions::new().write(true).create_new(true).open(&destination)?;
                std::io::copy(&mut input, &mut file)?;
            }
            Ok(destination)
        })();
        results.push((leaf, outcome));
    }
    let _ = fs::remove_dir_all(&scratch);
    Ok(results)
}

fn audio_plan(info: &MediaInfo, mode: VideoMode) -> StudioResult<AudioPlan> {
    let lossless = matches!(info.codec.as_str(), "flac" | "alac" | "ape") || info.codec.starts_with("pcm_");
    if mode == VideoMode::Mkv48 || (mode == VideoMode::Auto && info.rate >= 48000 && lossless) {
        let rate = info.rate.max(48000);
        let copy = rate == info.rate && info.bits >= 24 && matches!(info.codec.as_str(), "flac" | "pcm_s24le" | "pcm_s32le");
        let codec = if copy {
            info.codec.clone()
        } else if info.bits > 24 {
            "pcm_s32le".into()
        } else {
            "pcm_s24le".into()
        };
        let args = if copy {
            vec!["-c:a".into(), "copy".into()]
        } else {
            vec!["-c:a".into(), codec.clone(), "-ar".into(), rate.to_string()]
        };
        return Ok(AudioPlan {
            ext: "mkv",
            codec,
            rate,
            args,
            min_bits: 24,
        });
    }
    let rate = if matches!(info.rate, 32000 | 44100 | 48000) { info.rate } else { 48000 };
    Ok(AudioPlan {
        ext: "mp4",
        codec: "aac".into(),
        rate,
        args: vec![
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "256k".into(),
            "-ar".into(),
            rate.to_string(),
            "-movflags".into(),
            "+faststart".into(),
        ],
        min_bits: 0,
    })
}
