//! 调用程序旁边的裁剪版 ffmpeg / ffprobe. 不使用系统 PATH 上的完整 ffmpeg.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::error::{StudioError, StudioResult};

#[derive(Clone, Debug)]
pub struct Binaries {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

#[derive(Clone)]
pub struct Cancel {
    pub flag: Arc<AtomicBool>,
    pub child: Arc<Mutex<Option<Child>>>,
}

impl Default for Cancel {
    fn default() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
        }
    }
}

impl Cancel {
    pub fn reset(&self) {
        self.flag.store(false, Ordering::Relaxed);
    }

    pub fn request(&self) {
        self.flag.store(true, Ordering::Relaxed);
        if let Ok(mut slot) = self.child.lock() {
            if let Some(child) = slot.as_mut() {
                let _ = child.kill();
            }
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

pub fn locate() -> StudioResult<Binaries> {
    let ffmpeg_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let probe_name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            dirs.push(dir.join("vendor"));
        }
    }
    dirs.push(PathBuf::from("vendor"));
    for dir in dirs {
        let ffmpeg = dir.join(ffmpeg_name);
        let ffprobe = dir.join(probe_name);
        if ffmpeg.is_file() && ffprobe.is_file() {
            return Ok(Binaries { ffmpeg, ffprobe });
        }
    }
    Err(StudioError::new(
        "找不到裁剪版 ffmpeg 和 ffprobe. 请使用发行包, 或把 Actions 编好的两个文件放在程序同目录.",
    ))
}

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub duration: Option<f64>,
    pub channels: u32,
    pub rate: u32,
    pub layout: String,
    pub codec: String,
    pub bits: u32,
    pub tags: std::collections::BTreeMap<String, String>,
    pub attached_pic: bool,
    pub video_codec: String,
    pub width: u32,
    pub height: u32,
}

pub fn probe(binaries: &Binaries, path: &Path, cancel: &Cancel) -> StudioResult<MediaInfo> {
    let output = run(
        &binaries.ffprobe,
        &[
            "-v".into(),
            "error".into(),
            "-show_format".into(),
            "-show_streams".into(),
            "-of".into(),
            "json".into(),
            path.display().to_string(),
        ],
        false,
        None,
        cancel,
    )?;
    parse_probe(&output)
}

fn parse_probe(text: &str) -> StudioResult<MediaInfo> {
    let json: Value = serde_json::from_str(text).map_err(|error| StudioError::new(format!("无法读取音频信息: {error}")))?;
    let streams = json.get("streams").and_then(Value::as_array);
    let audio = streams
        .and_then(|items| items.iter().find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio")))
        .ok_or_else(|| StudioError::new("文件没有可用音轨."))?;
    let duration = as_f64(audio.get("duration")).or_else(|| as_f64(json.get("format").and_then(|format| format.get("duration"))));
    let channels = audio.get("channels").and_then(Value::as_u64).unwrap_or(0) as u32;
    let rate = as_f64(audio.get("sample_rate")).unwrap_or(0.0) as u32;
    if channels == 0 || rate == 0 {
        return Err(StudioError::new("无法读取声道数或采样率."));
    }
    let mut tags = std::collections::BTreeMap::new();
    collect_tags(audio.get("tags"), &mut tags);
    collect_tags(json.get("format").and_then(|format| format.get("tags")), &mut tags);
    let attached = streams.is_some_and(|items| {
        items.iter().any(|stream| {
            let flag = stream.get("disposition").and_then(|item| item.get("attached_pic"));
            flag.and_then(Value::as_i64) == Some(1)
                || flag.and_then(Value::as_bool) == Some(true)
                || stream.get("codec_type").and_then(Value::as_str) == Some("attachment")
        })
    });
    let video = streams.and_then(|items| {
        items.iter().find(|stream| {
            stream.get("codec_type").and_then(Value::as_str) == Some("video")
                && stream.get("disposition").and_then(|item| item.get("attached_pic")).and_then(Value::as_i64) != Some(1)
        })
    });
    Ok(MediaInfo {
        duration,
        channels,
        rate,
        layout: audio.get("channel_layout").and_then(Value::as_str).unwrap_or("").to_string(),
        codec: audio.get("codec_name").and_then(Value::as_str).unwrap_or("").to_string(),
        bits: audio
            .get("bits_per_raw_sample")
            .and_then(as_f64)
            .or_else(|| audio.get("bits_per_sample").and_then(as_f64))
            .unwrap_or(0.0) as u32,
        tags,
        attached_pic: attached,
        video_codec: video.and_then(|stream| stream.get("codec_name")).and_then(Value::as_str).unwrap_or("").to_string(),
        width: video.and_then(|stream| stream.get("width")).and_then(Value::as_u64).unwrap_or(0) as u32,
        height: video.and_then(|stream| stream.get("height")).and_then(Value::as_u64).unwrap_or(0) as u32,
    })
}

fn collect_tags(value: Option<&Value>, tags: &mut std::collections::BTreeMap<String, String>) {
    let Some(object) = value.and_then(Value::as_object) else {
        return;
    };
    for (key, item) in object {
        if let Some(text) = item.as_str() {
            tags.insert(key.to_ascii_lowercase(), text.chars().take(350).collect());
        }
    }
}

fn as_f64(value: Option<&Value>) -> Option<f64> {
    value.and_then(|item| item.as_f64().or_else(|| item.as_str().and_then(|text| text.parse().ok())))
}

pub fn run(
    binary: &Path,
    args: &[String],
    progress: bool,
    on_progress: Option<async_channel::Sender<f64>>,
    cancel: &Cancel,
) -> StudioResult<String> {
    if cancel.is_cancelled() {
        return Err(StudioError::new("已停止处理."));
    }
    let mut command = Command::new(binary);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            StudioError::new(format!("无法启动 {}.", binary.display()))
        } else {
            StudioError::from(error)
        }
    })?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if let Ok(mut slot) = cancel.child.lock() {
        *slot = Some(child);
    }
    let stderr_task = std::thread::spawn(move || {
        stderr
            .map(|pipe| {
                let mut text = String::new();
                let reader = BufReader::new(pipe);
                for line in reader.lines().map_while(Result::ok) {
                    text.push_str(&line);
                    text.push('\n');
                    if text.len() > 8000 {
                        text = text[text.len() - 7000..].to_string();
                    }
                }
                text
            })
            .unwrap_or_default()
    });
    let mut output = String::new();
    if let Some(pipe) = stdout {
        for line in BufReader::new(pipe).lines().map_while(Result::ok) {
            if cancel.is_cancelled() {
                cancel.request();
                let _ = stderr_task.join();
                wait_child(cancel);
                return Err(StudioError::new("已停止处理."));
            }
            if progress {
                if let Some(value) = line.strip_prefix("out_time_us=") {
                    if let Ok(micros) = value.parse::<f64>() {
                        if let Some(sender) = &on_progress {
                            let _ = sender.try_send(micros / 1_000_000.0);
                        }
                    }
                }
            } else {
                output.push_str(&line);
                output.push('\n');
            }
        }
    }
    let errors = stderr_task.join().unwrap_or_default();
    let status = wait_child(cancel)?;
    if cancel.is_cancelled() {
        return Err(StudioError::new("已停止处理."));
    }
    if !status.success() {
        let detail = errors.trim();
        return Err(StudioError::new(if detail.is_empty() {
            format!("ffmpeg 执行失败 ({})", status.code().unwrap_or(-1))
        } else {
            detail.to_string()
        }));
    }
    Ok(output)
}

fn wait_child(cancel: &Cancel) -> StudioResult<std::process::ExitStatus> {
    let child = cancel.child.lock().ok().and_then(|mut slot| slot.take());
    match child {
        Some(mut child) => child.wait().map_err(StudioError::from),
        None => Err(StudioError::new("ffmpeg 进程已经结束.")),
    }
}
