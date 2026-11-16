//! 锁住旧版 Electron 的行为, 以及已经修过的问题. 这些测试不调用 ffmpeg.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::cue;
use crate::effects::filter_chain;
use crate::export::{self, Job, Plan};
use crate::ffmpeg::{self, MediaInfo};
use crate::llm::{self, LlmConfig};
use crate::model::{self, LogEntry, Project, ProjectFile, ProjectImport, Settings, Tags, Track, Workspace};
use crate::names::{self, album_folder_name, apply_session_tags, batch_session_tags, filename_for, joined_end, track_tags};
use crate::video::{self, VideoMode};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("crs-parity-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn media(duration: f64, rate: u32) -> MediaInfo {
    MediaInfo {
        duration: Some(duration),
        channels: 2,
        rate,
        layout: "stereo".into(),
        codec: "flac".into(),
        bits: 24,
        tags: BTreeMap::new(),
        attached_pic: false,
        video_codec: String::new(),
        width: 0,
        height: 0,
    }
}

fn recital() -> Project {
    let mut project = Project::blank();
    project.name = "New Recital".into();
    project.date = "2026-11-13".into();
    project.performer = "Michel Dalberto".into();
    project.conductor = "Example Conductor".into();
    project.composer = "Beethoven / Debussy".into();
    project.orchestra = "Example Orchestra".into();
    project.sources = vec!["Original.wav".into()];
    project.tracks = vec![Track::blank(), Track::blank()];
    project.tracks[0].id = "first".into();
    project.tracks[0].source = "Original.wav".into();
    project.tracks[0].start = "00:00".into();
    project.tracks[0].end = "01:00".into();
    project.tracks[0].tags = Tags {
        album: "Obsolete".into(),
        artist: "Previous".into(),
        title: "Sonata".into(),
        composer: "Beethoven".into(),
        track: "09".into(),
        ..Tags::default()
    };
    project.tracks[1].id = "second".into();
    project.tracks[1].source = "Original.wav".into();
    project.tracks[1].start = "01:00".into();
    project.tracks[1].end = "02:00".into();
    project.tracks[1].tags.title = "Prélude".into();
    project.tracks[1].tags.date = "2001-04-03".into();
    let mut blank = Track::blank();
    blank.id = "empty".into();
    blank.start = "00:00".into();
    project.tracks.push(blank);
    project
}

#[test]
fn cue_times_match_the_75_fps_rounding() {
    let text = r#"
REM DATE 1989
PERFORMER "Pianist"
TITLE "Piano Recital"
FILE "Disc.flac" WAVE
  TRACK 01 AUDIO
    TITLE "Allegro"
    INDEX 00 00:00:00
    INDEX 01 00:02:01
  TRACK 02 AUDIO
    TITLE "Adagio"
    INDEX 01 03:10:74
FILE "Disc Two.flac" WAVE
  TRACK 03 AUDIO
    TITLE "Finale"
    PERFORMER "Other Pianist"
    INDEX 01 00:00:00
"#;
    let files = vec![
        ("Disc.flac".into(), "C:/rip/Disc.flac".into()),
        ("Disc Two.flac".into(), "C:/rip/Disc Two.flac".into()),
    ];
    let (album, tracks) = cue::parse_cue(text, &files).unwrap();
    assert_eq!(album.date, "1989");
    assert_eq!(album.artist, "Pianist");
    assert_eq!(album.title, "Piano Recital");
    assert_eq!(tracks[0].start, "00:00:02.013");
    assert_eq!(tracks[0].end, "00:03:10.987");
    assert_eq!(tracks[1].start, "00:03:10.987");
    assert!(tracks[1].end.is_empty());
    assert_eq!(tracks[2].start, "00:00:00.000");
    assert_eq!(tracks[2].artist, "Other Pianist");
    assert_eq!(tracks[0].filename, "01 - Allegro");
}

#[test]
fn cue_matches_a_stem_and_reports_a_missing_file() {
    let text = "FILE \"Disc.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\n";
    let files = vec![("Disc.flac".into(), "C:/rip/Disc.flac".into())];
    let (_, tracks) = cue::parse_cue(text, &files).unwrap();
    assert_eq!(tracks[0].source, "Disc.flac");
    let missing = cue::parse_cue("FILE \"Disc Two.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\n", &files).unwrap_err();
    assert!(missing.to_string().contains("Disc Two"));
    let invalid = cue::parse_cue("FILE \"Disc.flac\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:75\n", &files).unwrap_err();
    assert!(invalid.to_string().contains("时间无效"));
}

#[test]
fn cue_accepts_utf16_and_gbk() {
    let utf16 = "TITLE \"Recital\"\nFILE \"a.flac\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\n";
    let mut bytes = vec![0xFF, 0xFE];
    for unit in utf16.encode_utf16() {
        bytes.extend(unit.to_le_bytes());
    }
    let text = cue::decode_cue(&bytes);
    assert!(text.contains("Recital"));
    let (encoded, _, _) = encoding_rs::GBK.encode("标题");
    let text = cue::decode_cue(&encoded);
    assert!(text.contains("标题"));
}

#[test]
fn cd_text_keeps_the_old_album_and_track_fixes() {
    let text = "\
**Album Title:** Historical Piano Recordings
Album Artist: Leo Sirota
专辑作曲家: Frédéric Chopin
发行年份: １９６８
Album Artist: Unknown
01 - Allegro 5:30
03. Allegro 2:20
03. Presto
";
    let listed = cue::parse_track_list(text);
    assert_eq!(listed.album.title, "Historical Piano Recordings");
    assert_eq!(listed.album.artist, "Leo Sirota");
    assert_eq!(listed.album.composer, "Frédéric Chopin");
    assert_eq!(listed.album.date, "1968");
    assert_eq!(listed.numbered.get(&1).unwrap().0, "Allegro");
    assert!(listed.numbered.get(&2).is_none());
    assert_eq!(listed.numbered.get(&3).unwrap().0, "Allegro");
    assert_eq!(listed.issues.len(), 1);
    let dated = cue::parse_track_list("Album Date: 1982-11-15\nAlbum Artist: Unknown\n");
    assert_eq!(dated.album.date, "1982-11-15");
    assert_eq!(dated.album.artist, "");
    let composer = cue::parse_track_list("02. Kinderszenen | Composer: Robert Schumann\n");
    assert_eq!(composer.numbered.get(&2).unwrap().1, "Robert Schumann");
}

#[test]
fn cd_filenames_sort_with_numbers() {
    let mut names = vec!["Track10.flac".into(), "Track2.flac".into(), "Track1.flac".into()];
    cue::sort_cd_names(&mut names);
    assert_eq!(names, vec!["Track1.flac", "Track2.flac", "Track10.flac"]);
}

#[test]
fn session_tags_keep_track_composer_and_replace_date() {
    let mut project = recital();
    let batch = batch_session_tags(&project);
    assert!(batch.has_session);
    assert_eq!(batch.matched, 2);
    apply_session_tags(&mut project);
    assert_eq!(project.tracks[0].tags.track, "09");
    assert_eq!(project.tracks[0].tags.composer, "Beethoven");
    assert_eq!(project.tracks[1].tags.composer, "");
    assert_eq!(project.tracks[1].tags.track, "02");
    assert_eq!(project.tracks[1].tags.date, "2026-11-13");
    let first = track_tags(&project, &project.tracks[0], 0);
    assert_eq!(first.title, "Sonata");
    assert_eq!(first.artist, "Michel Dalberto");
    assert_eq!(first.album, "New Recital");
    assert_eq!(first.track, "09");
    assert_eq!(first.date, "2026-11-13");
    assert_eq!(first.composer, "Beethoven");
    assert_eq!(first.conductor, "Example Conductor");
    assert_eq!(first.soloist, "Michel Dalberto");
    assert_eq!(first.orchestra, "Example Orchestra");
    let second = track_tags(&project, &project.tracks[1], 1);
    assert_eq!(second.date, "2026-11-13");
    assert_eq!(second.composer, "");
    let mut empty = Project::blank();
    empty.tracks = project.tracks.clone();
    assert!(!batch_session_tags(&empty).has_session);
    empty.composer = "Debussy".into();
    let again = batch_session_tags(&empty);
    let fields = again.updates.iter().find(|(id, _)| id == "second").unwrap();
    assert!(fields.1.iter().any(|field| field.key == "Composer" && field.after == "Debussy"));
    assert_eq!(names::session_tag_values(&project).composer, "");
    let mut joined = project.clone();
    joined.composer = "A & B".into();
    assert_eq!(names::session_tag_values(&joined).composer, "");
    joined.composer = "A&B".into();
    assert_eq!(names::session_tag_values(&joined).composer, "A&B");
}

#[test]
fn export_plan_keeps_the_old_guards() {
    let scratch = Scratch::new();
    let audio = scratch.file("Original.wav", b"wav");
    let cover = scratch.file("cover.png", b"png");
    let mut project = recital();
    project.tracks.pop();
    project.source_paths.insert("Original.wav".into(), audio.display().to_string());
    project.settings.cover = false;
    project.settings.metadata = true;
    project.auto_join = true;
    project.tracks[0].end.clear();
    apply_session_tags(&mut project);
    let plan = export::prepare(&project).unwrap();
    assert_eq!(plan.jobs[0].tags.track, "09");
    assert_eq!(plan.jobs[0].end.unwrap(), 60.0);
    project.name.clear();
    assert!(export::prepare(&project).unwrap_err().to_string().contains("专辑名"));
    project.name = "New Recital".into();
    project.settings.format = "MP3".into();
    project.settings.rate = 96000;
    assert!(export::prepare(&project).unwrap_err().to_string().contains("MP3"));
    project.settings.format = "WAV".into();
    project.settings.rate = 48000;
    project.settings.cover = true;
    project.cover_file = "cover.png".into();
    project.cover_path = cover.display().to_string();
    let plan = export::prepare(&project).unwrap();
    assert_eq!(plan.format, "FLAC");
    assert!(plan.note.contains("FLAC"));
    project.settings.format = "FLAC".into();
    project.settings.cover = false;
    project.tracks[0].end = "00:00".into();
    assert!(export::prepare(&project).unwrap_err().to_string().contains("结束"));
    project.tracks[0].end = "01:00".into();
    project.tracks[0].tags.title.clear();
    assert!(export::prepare(&project).unwrap_err().to_string().contains("Title"));
    project.tracks[0].tags.title = "Sonata".into();
    project.tracks[0].filename = "Same".into();
    project.tracks[1].filename = "Same".into();
    assert!(export::prepare(&project).unwrap_err().to_string().contains("重复"));
    project.output = "..".into();
    project.auto_output = false;
    assert!(album_folder_name(&project).is_err());
}

#[test]
fn quality_check_matches_the_old_tolerances() {
    let mut plan = Plan {
        folder_name: "Album".into(),
        format: "FLAC".into(),
        settings: Settings::default(),
        jobs: vec![Job {
            order: 1,
            source: PathBuf::from("a.wav"),
            source_name: "a.wav".into(),
            target: "01 - Sonata.flac".into(),
            start: 0.0,
            end: Some(10.0),
            tags: Tags {
                title: "Sonata".into(),
                track: "09".into(),
                composer: "Beethoven".into(),
                ..Tags::default()
            },
        }],
        cover: Some(PathBuf::from("cover.png")),
        back_cover: None,
        note: String::new(),
    };
    plan.settings.rate = 48000;
    plan.settings.cover = true;
    plan.settings.metadata = true;
    let mut info = media(10.0, 48000);
    info.tags.insert("title".into(), "Sonata".into());
    info.tags.insert("track".into(), "9".into());
    info.tags.insert("composer".into(), "Beethoven".into());
    info.attached_pic = true;
    export::verify_output(&info, &plan, &plan.jobs[0], Some(10.2)).unwrap();
    info.duration = Some(10.4);
    assert!(export::verify_output(&info, &plan, &plan.jobs[0], Some(10.0)).unwrap_err().to_string().contains("时长"));
    info.duration = Some(10.0);
    info.rate = 44100;
    assert!(export::verify_output(&info, &plan, &plan.jobs[0], Some(10.0)).unwrap_err().to_string().contains("采样率"));
    info.rate = 48000;
    info.attached_pic = false;
    assert!(export::verify_output(&info, &plan, &plan.jobs[0], Some(10.0)).unwrap_err().to_string().contains("封面"));
    info.attached_pic = true;
    info.tags.insert("title".into(), "Other".into());
    assert!(export::verify_output(&info, &plan, &plan.jobs[0], Some(10.0)).unwrap_err().to_string().contains("Title"));
    plan.format = "WAV".into();
    info.tags.insert("title".into(), "Sonata".into());
    info.tags.insert("composer".into(), "Wrong".into());
    export::verify_output(&info, &plan, &plan.jobs[0], Some(10.0)).unwrap();
}

#[test]
fn artwork_archive_rejects_a_different_cover() {
    let scratch = Scratch::new();
    let cover = scratch.file("source.png", b"one");
    export::archive_artwork(&scratch.0, Some(&cover), false).unwrap();
    export::archive_artwork(&scratch.0, Some(&cover), false).unwrap();
    let other = scratch.file("other.png", b"two");
    let error = export::archive_artwork(&scratch.0, Some(&other), false).unwrap_err();
    assert!(error.to_string().contains("已有不同"));
    assert!(scratch.0.join("cover.png").is_file());
    let jpeg = scratch.file("back.jpeg", b"jpg");
    export::archive_artwork(&scratch.0, Some(&jpeg), true).unwrap();
    assert!(scratch.0.join("back.jpg").is_file());
}

#[test]
fn probe_reads_attached_pictures_and_string_durations() {
    let json = r#"{
        "streams": [
            {"codec_type":"audio","duration":"12.5","channels":2,"sample_rate":"48000","codec_name":"flac","bits_per_raw_sample":"24","tags":{"Title":"Sonata"}},
            {"codec_type":"video","codec_name":"mjpeg","width":600,"height":600,"disposition":{"attached_pic":1}}
        ]
    }"#;
    let info = ffmpeg::parse_probe(json).unwrap();
    assert_eq!(info.duration, Some(12.5));
    assert_eq!(info.rate, 48000);
    assert_eq!(info.bits, 24);
    assert!(info.attached_pic);
    assert!(info.video_codec.is_empty());
    assert_eq!(info.tags.get("title").map(String::as_str), Some("Sonata"));
    let flagged = r#"{"streams":[{"codec_type":"audio","duration":1,"channels":1,"sample_rate":44100,"disposition":{"attached_pic":true}}]}"#;
    assert!(ffmpeg::parse_probe(flagged).unwrap().attached_pic);
    let attachment = r#"{"streams":[{"codec_type":"audio","duration":1,"channels":1,"sample_rate":44100},{"codec_type":"attachment"}]}"#;
    assert!(ffmpeg::parse_probe(attachment).unwrap().attached_pic);
    assert!(ffmpeg::parse_probe(r#"{"streams":[]}"#).is_err());
}

#[test]
fn sidecar_lookup_ignores_any_other_directory() {
    let scratch = Scratch::new();
    let other = Scratch::new();
    let ffmpeg_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let probe_name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    other.file(ffmpeg_name, b"nope");
    other.file(probe_name, b"nope");
    let error = ffmpeg::locate_in(&[scratch.0.clone()]).unwrap_err();
    assert!(error.to_string().contains("裁剪版"));
    scratch.file(ffmpeg_name, b"ffmpeg");
    assert!(ffmpeg::locate_in(&[scratch.0.clone()]).is_err());
    scratch.file(probe_name, b"ffprobe");
    let found = ffmpeg::locate_in(&[scratch.0.clone()]).unwrap();
    assert_eq!(found.ffmpeg, scratch.0.join(ffmpeg_name));
    assert_eq!(found.ffprobe, scratch.0.join(probe_name));
}

#[test]
fn cover_video_picks_the_old_container() {
    let mut info = media(30.0, 96000);
    info.codec = "flac".into();
    info.bits = 24;
    let (ext, codec, rate) = video::cover_audio_choice(&info, VideoMode::Auto).unwrap();
    assert_eq!((ext, codec.as_str(), rate), ("mkv", "flac", 96000));
    info.rate = 44100;
    info.codec = "pcm_s16le".into();
    info.bits = 16;
    let (ext, codec, rate) = video::cover_audio_choice(&info, VideoMode::Auto).unwrap();
    assert_eq!((ext, codec.as_str(), rate), ("mp4", "aac", 44100));
    let (ext, codec, rate) = video::cover_audio_choice(&info, VideoMode::Mkv48).unwrap();
    assert_eq!((ext, codec.as_str(), rate), ("mkv", "pcm_s24le", 48000));
    info.video_codec = "h264".into();
    info.width = 1920;
    info.height = 1080;
    assert!(video::cover_frame_accepted(&info));
    info.height = 720;
    assert!(!video::cover_frame_accepted(&info));
}

#[test]
fn effect_chain_keeps_the_previous_order() {
    let mut settings = Settings::default();
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
    settings.effects.fade_in = 0.2;
    settings.effects.fade_out = 0.3;
    settings.effects.limiter = -1.0;
    let chain = filter_chain(&settings, Some(3.75), 2).unwrap();
    let names = chain.iter().map(|item| item.split(['=', ':']).next().unwrap()).collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "highpass", "lowpass", "equalizer", "equalizer", "equalizer", "adeclip", "adeclick", "afftdn", "bass", "equalizer", "treble", "agate", "pan", "deesser",
            "acompressor", "loudnorm", "afade", "afade", "alimiter"
        ]
    );
    assert_eq!(chain[2], "equalizer=f=50:t=q:w=20:g=-24");
    assert!(chain[17].contains("st=3.450000"));
    assert!(chain.last().unwrap().contains("level=false"));
    settings.effects.channels = "swap".into();
    assert!(filter_chain(&settings, Some(3.75), 1).is_err());
    settings.effects.lowpass = 40.0;
    settings.effects.channels = "source".into();
    assert!(filter_chain(&settings, Some(3.75), 2).is_err());
}

#[test]
fn english_review_skips_dates_empty_fields_and_chinese() {
    let mut project = Project::blank();
    project.date = "2026-11-06".into();
    project.city = "北京".into();
    project.venue = "上海交响音乐厅".into();
    project.performer = "瓦尔荣".into();
    project.composer = "贝多芬".into();
    let raw = r#"{"fields":{"date":{"value":"2020-01-01"},"city":{"value":"Beijing"},"venue":{"value":"Shanghai Symphony Hall"},"performer":{"value":"Dénes Várjon"},"composer":{"value":"贝多芬"},"orchestra":{"value":"Shanghai Symphony Orchestra"}}}"#;
    let output = llm::interpret_english(raw, &project).unwrap();
    assert_eq!(output.len(), 3);
    assert!(output.iter().any(|item| item.key == "city" && item.after == "Beijing" && item.apply));
    assert!(output.iter().any(|item| item.key == "venue" && item.after == "Shanghai Symphony Hall" && item.apply));
    assert!(output.iter().any(|item| item.key == "performer" && item.after == "Dénes Várjon" && item.apply));
    assert!(output.iter().all(|item| item.key != "composer" && item.key != "orchestra"));
    let fenced = llm::interpret_english("```json\n{\"name\":{\"value\":\"Recital\",\"note\":\"ok\"}}\n```", &{
        let mut named = project.clone();
        named.name = "独奏会".into();
        named
    })
    .unwrap();
    assert_eq!(fenced[0].after, "Recital");
    llm::apply_field(&mut project, "date", "1999-01-01");
    assert_eq!(project.date, "2026-11-06");
    llm::apply_field(&mut project, "city", "Shanghai");
    assert_eq!(project.city, "Shanghai");
}

#[test]
fn llm_config_does_not_enter_the_project_file() {
    let previous = LlmConfig {
        base_url: "https://api.deepseek.com".into(),
        model: "deepseek-chat".into(),
        api_key: "secret-key".into(),
    };
    let changed = llm::config_after_endpoint_change(
        &previous,
        LlmConfig {
            base_url: "https://service.example/v1".into(),
            model: "other".into(),
            api_key: "secret-key".into(),
        },
    );
    assert!(changed.api_key.is_empty());
    let same = llm::config_after_endpoint_change(
        &previous,
        LlmConfig {
            base_url: "https://api.deepseek.com/".into(),
            model: "deepseek-chat".into(),
            api_key: "secret-key".into(),
        },
    );
    assert_eq!(same.api_key, "secret-key");
    assert!(llm::remote_base_allowed("https://service.example/v1").is_ok());
    assert!(llm::remote_base_allowed("http://127.0.0.1:8000/v1").is_ok());
    assert!(llm::remote_base_allowed("http://localhost:9000").is_ok());
    assert!(llm::remote_base_allowed("http://service.example/v1").unwrap_err().to_string().contains("HTTPS"));
    let refused = llm::chat(
        &LlmConfig {
            base_url: "http://service.example/v1".into(),
            model: "example".into(),
            api_key: "secret-key".into(),
        },
        "system",
        "user",
    )
    .unwrap_err();
    assert!(refused.to_string().contains("HTTPS"));
    let ids = llm::model_ids(&serde_json::json!({"data":[{"id":"example-vision"},{"id":"example-vision"},{"id":"other"}]})).unwrap();
    assert_eq!(ids, vec!["example-vision", "other"]);
    let mut project = Project::blank();
    project.name = "Recital".into();
    project.notes = "secret-key stays out of the schema".into();
    let file = ProjectFile::from_project(&project, &[]);
    let text = serde_json::to_string(&file).unwrap();
    assert!(text.contains("\"format\":\"classical-recording-studio/project\""));
    assert!(text.contains("\"coverFile\""));
    assert!(!text.contains("apiKey"));
    assert!(!text.contains("api_key"));
}

#[test]
fn workspace_round_trip_and_same_id_copy() {
    let mut project = Project::blank();
    project.name = "Recital".into();
    let other = Project::blank();
    let mine = LogEntry::new(&project, "导出", "成功", "done");
    let foreign = LogEntry::new(&other, "导出", "成功", "elsewhere");
    let file = ProjectFile::from_project(&project, &[mine, foreign]);
    assert_eq!(file.logs.len(), 1);
    let mut workspace = Workspace::default();
    assert_eq!(model::import_project(&mut workspace, file.clone()), ProjectImport::New);
    assert_eq!(model::import_project(&mut workspace, file.clone()), ProjectImport::Existing);
    let mut changed = file.clone();
    changed.project.name = "Different".into();
    assert_eq!(model::import_project(&mut workspace, changed), ProjectImport::Copy);
    assert_eq!(workspace.projects.len(), 2);
    assert_ne!(workspace.projects[0].id, workspace.projects[1].id);
    let saved = serde_json::to_string(&workspace).unwrap();
    let loaded: Workspace = serde_json::from_str(&saved).unwrap();
    assert_eq!(loaded.projects.len(), 2);
    assert_eq!(loaded.templates.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), ["archive-flac", "flac-cover", "gain-width"]);
    assert!(!loaded.templates[1].settings.split);
    assert_eq!(loaded.templates[2].settings.gain, 16.0);
    assert_eq!(loaded.templates[2].settings.width, 130.0);
    assert!(!loaded.templates[2].settings.cover);
    let scratch = Scratch::new();
    let path = scratch.0.join("project.crstudio.json");
    crate::store::write_project_file(&path, &file).unwrap();
    let read = crate::store::read_project_file(&path).unwrap();
    assert_eq!(read.project.name, "Recital");
    std::fs::write(scratch.0.join("bad.json"), r#"{"format":"classical-recording-studio/project","version":9,"project":{"id":"x"}}"#).unwrap();
    assert!(crate::store::read_project_file(&scratch.0.join("bad.json")).is_err());
}

#[test]
fn names_and_catalog_keep_earlier_cases() {
    assert_eq!(names::time_seconds("1:02:03.40").unwrap(), Some(3723.4));
    assert!(names::time_seconds("1:70").is_err());
    let mut tracks = vec![Track::blank(), Track::blank()];
    tracks[0].source = "live.wav".into();
    tracks[0].start = "00:10".into();
    tracks[1].source = "live.wav".into();
    tracks[1].start = "01:10".into();
    assert_eq!(joined_end(&tracks, 0, true), "01:10");
    assert_eq!(joined_end(&tracks, 0, false), "");
    let tags = Tags { title: "Sonata: I".into(), track: "1".into(), ..Tags::default() };
    assert_eq!(filename_for("{Track} - {Title}", &tracks[0], &tags, 0), "1 - Sonata - I");
    assert!(!names::output_folder_valid("con"));
    assert!(!names::cover_name("cover.heic"));
    assert!(names::cover_name("cover.PNG"));
    let hit = crate::catalog::suggest_work("贝五").unwrap();
    assert!(hit.title.contains("Symphony No. 5"));
    let moonlight = crate::catalog::suggest_work("今晚的月光奏鸣曲").unwrap();
    assert!(moonlight.title.contains("Moonlight"));
}
