//! 调用裁剪版 ffmpeg / ffprobe 的测试. Actions 在下载 sidecar 后设置 CRS_FFMPEG 和 CRS_FFPROBE.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use classical_recording_studio::effects::filter_chain;
use classical_recording_studio::export::{self, Job, Plan};
use classical_recording_studio::ffmpeg::{self, Binaries, Cancel, MediaInfo};
use classical_recording_studio::model::{Settings, Tags};
use classical_recording_studio::video::{self, VideoMode};

const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xDB, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let tick = SystemTime::now().duration_since(UNIX_EPOCH).map(|item| item.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("crs-media-{tick}"));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bins() -> Binaries {
    Binaries {
        ffmpeg: PathBuf::from(std::env::var("CRS_FFMPEG").expect("CRS_FFMPEG")),
        ffprobe: PathBuf::from(std::env::var("CRS_FFPROBE").expect("CRS_FFPROBE")),
    }
}

fn cancel() -> Cancel {
    Cancel::default()
}

fn progress() -> async_channel::Sender<String> {
    async_channel::unbounded().0
}

fn write_tone(path: &Path, rate: u32, channels: u16, seconds: f64, freq: f64, amplitude: f64, bits: u16) {
    assert!(matches!(bits, 16 | 24));
    let frames = (seconds * f64::from(rate)).round() as usize;
    let width = (bits / 8) as usize;
    let mut pcm = Vec::with_capacity(frames * channels as usize * width);
    for index in 0..frames {
        let t = index as f64 / f64::from(rate);
        let sample = (t * freq * std::f64::consts::TAU).sin() * amplitude;
        let full = if bits == 16 { 32767.0 } else { 8_388_607.0 };
        let value = (sample.clamp(-1.0, 1.0) * full).round() as i32;
        let bytes = value.to_le_bytes();
        for _ in 0..channels {
            pcm.extend_from_slice(&bytes[..width]);
        }
    }
    let data_len = pcm.len() as u32;
    let block = channels * (bits / 8);
    let byte_rate = rate * u32::from(block);
    let mut header = Vec::with_capacity(44);
    header.extend(b"RIFF");
    header.extend((36 + data_len).to_le_bytes());
    header.extend(b"WAVEfmt ");
    header.extend(16u32.to_le_bytes());
    header.extend(1u16.to_le_bytes());
    header.extend(channels.to_le_bytes());
    header.extend(rate.to_le_bytes());
    header.extend(byte_rate.to_le_bytes());
    header.extend(block.to_le_bytes());
    header.extend(bits.to_le_bytes());
    header.extend(b"data");
    header.extend(data_len.to_le_bytes());
    let mut file = fs::File::create(path).unwrap();
    std::io::Write::write_all(&mut file, &header).unwrap();
    std::io::Write::write_all(&mut file, &pcm).unwrap();
}

fn encode(bins: &Binaries, args: &[&str], output: &Path) {
    let mut owned: Vec<String> = ["-hide_banner", "-nostdin", "-y", "-v", "error"].into_iter().map(str::to_string).collect();
    owned.extend(args.iter().map(|item| (*item).to_string()));
    owned.push(output.display().to_string());
    ffmpeg::run(&bins.ffmpeg, &owned, false, None, &cancel()).unwrap_or_else(|error| panic!("{error}"));
}

fn probe(bins: &Binaries, path: &Path) -> MediaInfo {
    ffmpeg::probe(bins, path, &cancel()).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn decode_s16(bins: &Binaries, input: &Path, scratch: &Scratch) -> Vec<i16> {
    let output = scratch.path(&format!("pcm-{}.wav", input.file_stem().unwrap_or_default().to_string_lossy()));
    encode(bins, &["-i", &input.display().to_string(), "-map", "0:a:0", "-c:a", "pcm_s16le"], &output);
    let bytes = fs::read(&output).unwrap();
    let data = bytes.windows(4).position(|mark| mark == b"data").expect("wav data");
    let size = u32::from_le_bytes(bytes[data + 4..data + 8].try_into().unwrap()) as usize;
    bytes[data + 8..data + 8 + size].chunks_exact(2).map(|item| i16::from_le_bytes([item[0], item[1]])).collect()
}

fn peak_db(samples: &[i16]) -> f64 {
    let max = samples.iter().map(|sample| f64::from(sample.unsigned_abs())).fold(0.0, f64::max);
    if max == 0.0 {
        -120.0
    } else {
        20.0 * (max / 32768.0).log10()
    }
}

fn average(samples: &[i16], from: usize, to: usize) -> f64 {
    let slice = &samples[from..to];
    slice.iter().map(|sample| f64::from(sample.unsigned_abs())).sum::<f64>() / slice.len() as f64
}

fn job(source: &Path, target: &str, start: f64, end: Option<f64>, title: &str) -> Job {
    Job {
        order: 1,
        source: source.to_path_buf(),
        source_name: source.file_name().unwrap_or_default().to_string_lossy().to_string(),
        target: target.into(),
        start,
        end,
        tags: Tags { title: title.into(), ..Tags::default() },
        disc: 0,
        segments: Vec::new(),
    }
}

fn plan(folder: &str, settings: Settings, jobs: Vec<Job>, cover: Option<PathBuf>) -> Plan {
    Plan {
        folder_name: folder.into(),
        format: settings.format.clone(),
        settings,
        jobs,
        cover,
        back_cover: None,
        note: String::new(),
    }
}

fn export_ok(bins: &Binaries, plan: &Plan, parent: &Path) -> PathBuf {
    let reports = export::export_album(bins, plan, parent, &cancel(), &progress()).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(reports.len(), 1, "{reports:?}");
    assert!(reports[0].error.is_none(), "{:?}", reports[0].error);
    reports[0].path.clone().unwrap()
}

#[test]
fn concatenate_cuts_and_embeds_a_cover() {
    let bins = bins();
    let scratch = Scratch::new();
    let first = scratch.path("第一段.wav");
    let second = scratch.path("second.wav");
    let cover = scratch.path("cover.png");
    write_tone(&first, 44100, 1, 1.0, 330.0, 0.4, 16);
    write_tone(&second, 48000, 2, 1.5, 660.0, 0.4, 16);
    fs::write(&cover, PNG_1X1).unwrap();
    let joined = scratch.path("joined.flac");
    let media = export::concatenate(&bins, &[first.clone(), second], &joined, &cancel()).unwrap();
    assert_eq!(media.rate, 48000);
    assert!((media.duration.unwrap() - 2.5).abs() < 0.05, "{}", media.duration.unwrap());

    let preview = export::preview_span(&bins, &joined, 1.0, 0.4, &[], 48000, &cancel()).unwrap();
    let heard = probe(&bins, &preview);
    assert!((heard.duration.unwrap() - 0.4).abs() < 0.05);
    let _ = fs::remove_file(&preview);

    let original = fs::read(&first).unwrap();
    let mut settings = Settings::default();
    settings.format = "FLAC".into();
    settings.rate = 48000;
    settings.gain = 2.0;
    settings.width = 100.0;
    settings.cover = true;
    let made = plan("Album", settings, vec![job(&joined, "01 - Allegro.flac", 0.5, Some(2.0), "Allegro")], Some(cover));
    let exported = export_ok(&bins, &made, &scratch.0);
    let info = probe(&bins, &exported);
    assert!((info.duration.unwrap() - 1.5).abs() < 0.05, "{}", info.duration.unwrap());
    assert!(info.attached_pic);
    assert_eq!(info.tags.get("title").map(String::as_str), Some("Allegro"));
    let again = export::export_album(&bins, &made, &scratch.0, &cancel(), &progress()).unwrap();
    assert!(again[0].error.is_none(), "{:?}", again[0].error);
    assert!(again[0].skipped);
    assert_eq!(fs::read(&first).unwrap(), original);

    made_out_of_range(&bins, &scratch, &joined);
}

fn made_out_of_range(bins: &Binaries, scratch: &Scratch, source: &Path) {
    let mut settings = Settings::default();
    settings.cover = false;
    settings.format = "FLAC".into();
    settings.rate = 48000;
    let late = plan("Late", settings, vec![job(source, "late.flac", 30.0, None, "Late")], None);
    let reports = export::export_album(bins, &late, &scratch.0, &cancel(), &progress()).unwrap();
    assert!(reports[0].error.as_deref().unwrap_or("").contains("超出"));
}

#[test]
fn effects_keep_duration_channels_and_the_source() {
    let bins = bins();
    let scratch = Scratch::new();
    let source = scratch.path("source.wav");
    write_tone(&source, 44100, 2, 4.0, 330.0, 0.35, 16);
    let original = fs::read(&source).unwrap();
    let mut settings = Settings::default();
    settings.format = "FLAC".into();
    settings.rate = 48000;
    settings.cover = false;
    settings.metadata = true;
    settings.effects.denoise = "fft".into();
    settings.effects.noise_reduction = 8.0;
    settings.effects.hum = "50".into();
    settings.effects.declip = true;
    settings.effects.declick = true;
    settings.effects.highpass = 30.0;
    settings.effects.lowpass = 16000.0;
    settings.effects.bass = 1.0;
    settings.effects.mid = -1.0;
    settings.effects.treble = 1.0;
    settings.effects.gate = -60.0;
    settings.effects.channels = "swap".into();
    settings.effects.compress = 2.0;
    settings.effects.threshold = -20.0;
    settings.effects.deess = true;
    settings.effects.normalize = "loud".into();
    settings.effects.lufs = -18.0;
    settings.effects.limiter = -1.0;
    settings.effects.fade_in = 0.2;
    settings.effects.fade_out = 0.3;
    let exported = export_ok(&bins, &plan("Effects", settings.clone(), vec![job(&source, "denoised.flac", 0.25, None, "Processed")], None), &scratch.0);
    let info = probe(&bins, &exported);
    assert_eq!(info.rate, 48000);
    assert_eq!(info.channels, 2);
    assert!((info.duration.unwrap() - 3.75).abs() < 0.1, "{}", info.duration.unwrap());
    assert_eq!(info.tags.get("title").map(String::as_str), Some("Processed"));
    assert_eq!(fs::read(&source).unwrap(), original);
    let samples = decode_s16(&bins, &exported, &scratch);
    let tail = samples.len().saturating_sub(4800);
    assert!(average(&samples, tail, samples.len()) < average(&samples, samples.len() / 3, samples.len() / 3 + 4800) * 0.25);

    settings.effects = classical_recording_studio::model::Effects::default();
    settings.effects.denoise = "nlm".into();
    settings.effects.channels = "mono".into();
    settings.effects.normalize = "dynamic".into();
    let mono = export_ok(&bins, &plan("Mono", settings, vec![job(&source, "mono.flac", 0.25, Some(2.0), "Mono")], None), &scratch.0);
    assert_eq!(probe(&bins, &mono).channels, 1);
}

#[test]
fn limiter_holds_a_hot_signal_near_minus_one() {
    let bins = bins();
    let scratch = Scratch::new();
    let source = scratch.path("hot.wav");
    write_tone(&source, 48000, 1, 1.0, 330.0, 0.8, 16);
    let original = fs::read(&source).unwrap();
    let mut open = Settings::default();
    open.format = "FLAC".into();
    open.rate = 48000;
    open.gain = 24.0;
    open.cover = false;
    open.metadata = false;
    let clipped = export_ok(&bins, &plan("Open", open.clone(), vec![job(&source, "open.flac", 0.0, Some(1.0), "")], None), &scratch.0);
    assert!(peak_db(&decode_s16(&bins, &clipped, &scratch)) > -0.05);

    open.effects.limiter = -1.0;
    let limited = export_ok(&bins, &plan("Limited", open.clone(), vec![job(&source, "limited.flac", 0.0, Some(1.0), "")], None), &scratch.0);
    let peak = peak_db(&decode_s16(&bins, &limited, &scratch));
    assert!((peak + 1.0).abs() < 0.2, "{peak}");
    let filters = filter_chain(&open, Some(1.0), 1).unwrap();
    let heard = export::preview_span(&bins, &source, 0.0, 1.0, &filters, 48000, &cancel()).unwrap();
    let preview_peak = peak_db(&decode_s16(&bins, &heard, &scratch));
    assert!((preview_peak + 1.0).abs() < 0.2, "{preview_peak}");
    assert_eq!(fs::read(&source).unwrap(), original);
}

#[test]
fn cover_videos_follow_the_packaging_mode() {
    let bins = bins();
    let scratch = Scratch::new();
    let cover = scratch.path("front.png");
    let high_wav = scratch.path("high.wav");
    let high = scratch.path("01 - Allegro.flac");
    let low = scratch.path("02 - Andante.wav");
    fs::write(&cover, PNG_1X1).unwrap();
    write_tone(&high_wav, 96000, 2, 1.2, 440.0, 0.3, 24);
    encode(&bins, &["-i", &high_wav.display().to_string(), "-c:a", "flac", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"], &high);
    write_tone(&low, 44100, 2, 1.2, 330.0, 0.3, 16);
    let source_bytes = fs::read(&high).unwrap();
    let high_info = probe(&bins, &high);
    assert!(high_info.bits >= 24, "flac bits {}", high_info.bits);
    let progress = progress();
    let batch = video::make_cover_videos(&bins, &[high.clone(), low.clone()], &cover, &scratch.0, VideoMode::Auto, &cancel(), &progress).unwrap();
    assert!(batch.iter().all(|(_, result)| result.is_ok()), "{batch:?}");
    let mkv = scratch.0.join("封面视频").join("01 - Allegro.mkv");
    let mp4 = scratch.0.join("封面视频").join("02 - Andante.mp4");
    let mkv_info = probe(&bins, &mkv);
    assert_eq!(mkv_info.codec, "flac");
    assert_eq!(mkv_info.rate, 96000);
    assert!(mkv_info.bits >= 24, "{}", mkv_info.bits);
    assert_eq!(video_size(&mkv_info), (1920, 1080));
    let source_pcm = decode_s16(&bins, &high, &scratch);
    let copied_pcm = decode_s16(&bins, &mkv, &scratch);
    assert_eq!(source_pcm, copied_pcm);
    let mp4_info = probe(&bins, &mp4);
    assert_eq!(mp4_info.codec, "aac");
    assert_eq!(mp4_info.rate, 44100);
    assert_eq!(video_size(&mp4_info), (1920, 1080));

    let upgrade = video::make_cover_videos(&bins, &[low.clone()], &cover, &scratch.0, VideoMode::Mkv48, &cancel(), &progress).unwrap();
    assert!(upgrade[0].1.is_ok(), "{:?}", upgrade[0].1);
    let upgraded = probe(&bins, &scratch.0.join("封面视频").join("02 - Andante.mkv"));
    assert_eq!(upgraded.codec, "pcm_s24le");
    assert_eq!(upgraded.rate, 48000);
    assert!(upgraded.bits >= 24, "{}", upgraded.bits);

    let repeated = video::make_cover_videos(&bins, &[low], &cover, &scratch.0, VideoMode::Mp4, &cancel(), &progress).unwrap();
    assert!(repeated[0].1.as_ref().unwrap_err().to_string().contains("已有同名"));
    assert_eq!(fs::read(&high).unwrap(), source_bytes);
    assert!(scratch_gone(&scratch.0));
}

#[test]
fn segmented_exports_keep_the_sum_of_the_spans() {
    let bins = bins();
    let scratch = Scratch::new();
    let first = scratch.path("left.wav");
    let second = scratch.path("right.wav");
    let long = scratch.path("long.wav");
    write_tone(&first, 48000, 2, 1.0, 440.0, 0.2, 16);
    write_tone(&second, 48000, 2, 1.0, 880.0, 0.2, 16);
    write_tone(&long, 48000, 2, 2.0, 440.0, 0.2, 16);
    let mut settings = Settings::default();
    settings.cover = false;
    settings.metadata = false;
    settings.format = "FLAC".into();
    settings.rate = 48000;

    let mut across = job(&first, "across.flac", 0.0, Some(1.0), "Across");
    across.segments = vec![
        export::SpanCut { source: first.clone(), start: 0.0, end: Some(1.0) },
        export::SpanCut { source: second.clone(), start: 0.0, end: Some(1.0) },
    ];
    let exported = export_ok(&bins, &plan("Box", settings.clone(), vec![across], None), &scratch.0);
    let info = probe(&bins, &exported);
    assert!((info.duration.unwrap() - 2.0).abs() < 0.3, "{}", info.duration.unwrap());
    let samples = decode_s16(&bins, &exported, &scratch);
    let expected = decode_s16(&bins, &first, &scratch).len() + decode_s16(&bins, &second, &scratch).len();
    assert!((samples.len() as i64 - expected as i64).unsigned_abs() < 24_000, "{} vs {expected}", samples.len());

    let mut gapped = job(&long, "gap.flac", 0.0, Some(0.5), "Gap");
    gapped.segments = vec![
        export::SpanCut { source: long.clone(), start: 0.0, end: Some(0.5) },
        export::SpanCut { source: long.clone(), start: 1.5, end: Some(2.0) },
    ];
    let gap_out = export_ok(&bins, &plan("GapAlbum", settings.clone(), vec![gapped], None), &scratch.0);
    let gap_info = probe(&bins, &gap_out);
    assert!((gap_info.duration.unwrap() - 1.0).abs() < 0.3, "{}", gap_info.duration.unwrap());

    let whole = job(&long, "whole.flac", 0.0, Some(2.0), "Whole");
    let whole_out = export_ok(&bins, &plan("Whole", settings.clone(), vec![whole], None), &scratch.0);
    let mut halves = job(&long, "halves.flac", 0.0, Some(1.0), "Halves");
    halves.segments = vec![
        export::SpanCut { source: long.clone(), start: 0.0, end: Some(1.0) },
        export::SpanCut { source: long, start: 1.0, end: Some(2.0) },
    ];
    let halves_out = export_ok(&bins, &plan("Halves", settings, vec![halves], None), &scratch.0);
    let whole_info = probe(&bins, &whole_out);
    let halves_info = probe(&bins, &halves_out);
    assert!((whole_info.duration.unwrap() - halves_info.duration.unwrap()).abs() < 0.3);
    let whole_len = decode_s16(&bins, &whole_out, &scratch).len();
    let halves_len = decode_s16(&bins, &halves_out, &scratch).len();
    assert!((whole_len as i64 - halves_len as i64).unsigned_abs() < 24_000, "{whole_len} vs {halves_len}");
}

fn video_size(info: &MediaInfo) -> (u32, u32) {
    assert_eq!(info.video_codec, "h264");
    (info.width, info.height)
}

fn scratch_gone(root: &Path) -> bool {
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".studio-video-") || name.starts_with(".studio-") {
                return false;
            }
            if entry.path().is_dir() {
                pending.push(entry.path());
            }
        }
    }
    true
}
