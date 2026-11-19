//! 乐章合并, 多碟命名, CUE 配对, 导出计划和重新关联. 这些测试不调用 ffmpeg.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::cue::{self, CueAlbum};
use crate::export::{self, Job, Plan, SpanCut};
use crate::ffmpeg::MediaInfo;
use crate::model::{Project, ProjectFile, Segment, Settings, Tags, Track};
use crate::names::{self, MergePlan, MovementGroup};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("crs-suite-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn track(source: &str, start: &str, end: &str, title: &str) -> Track {
    let mut track = Track::blank();
    track.source = source.into();
    track.start = start.into();
    track.end = end.into();
    track.tags.title = title.into();
    track
}

fn titled(titles: &[&str]) -> Vec<Track> {
    titles.iter().map(|title| track("live.wav", "00:00", "00:10", title)).collect()
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
        attached_pic: true,
        video_codec: String::new(),
        width: 0,
        height: 0,
    }
}

fn job(target: &str, segments: usize) -> Job {
    Job {
        order: 1,
        source: PathBuf::from("a.wav"),
        source_name: "a.wav".into(),
        target: target.into(),
        start: 0.0,
        end: Some(1.0),
        tags: Tags { title: "Sonata".into(), ..Tags::default() },
        disc: 0,
        segments: (0..segments).map(|_| SpanCut { source: PathBuf::from("a.wav"), start: 0.0, end: Some(1.0) }).collect(),
    }
}

fn plan(format: &str, jobs: Vec<Job>) -> Plan {
    let mut settings = Settings::default();
    settings.cover = false;
    settings.metadata = true;
    settings.rate = 48000;
    Plan {
        folder_name: "Album".into(),
        format: format.into(),
        settings,
        jobs,
        cover: None,
        back_cover: None,
        note: String::new(),
    }
}

fn recital(root: &Path, files: &[&str]) -> Project {
    let mut project = Project::blank();
    project.name = "Recital".into();
    project.settings.cover = false;
    project.settings.metadata = true;
    project.tracks.clear();
    for name in files {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap_or(root)).unwrap();
        std::fs::write(&path, b"wav").unwrap();
        project.sources.push((*name).into());
        project.source_paths.insert((*name).into(), path.display().to_string());
    }
    project
}

#[test]
fn merge_rejects_too_few_invalid_and_unsorted_choices() {
    let tracks = vec![track("a.wav", "00:00", "00:10", "I"), track("a.wav", "00:10", "00:20", "II")];
    assert!(names::plan_merge(&tracks, &[], &BTreeMap::new()).unwrap_err().to_string().contains("两段"));
    assert!(names::plan_merge(&tracks, &[0], &BTreeMap::new()).unwrap_err().to_string().contains("两段"));
    assert!(names::plan_merge(&tracks, &[0, 0], &BTreeMap::new()).unwrap_err().to_string().contains("两段"));
    assert!(names::plan_merge(&tracks, &[0, 4], &BTreeMap::new()).unwrap_err().to_string().contains("无效"));
    let mut bare = tracks.clone();
    bare[1].source.clear();
    assert!(names::plan_merge(&bare, &[0, 1], &BTreeMap::new()).unwrap_err().to_string().contains("录音"));
    bare[1].source = "a.wav".into();
    bare[1].parts = vec![Segment { source: String::new(), start: "00:00".into(), end: "00:01".into(), title: "x".into() }];
    assert!(names::plan_merge(&bare, &[0, 1], &BTreeMap::new()).unwrap_err().to_string().contains("片段"));
    bare[0].start = "nope".into();
    bare[1].parts.clear();
    assert!(names::plan_merge(&bare, &[0, 1], &BTreeMap::new()).unwrap_err().to_string().contains("时间点"));
}

#[test]
fn merge_collapses_only_adjacent_touches_and_keeps_titles() {
    let tracks = vec![
        track("a.wav", "00:00", "00:10", "I. Adagio"),
        track("a.wav", "00:10", "00:20", "II. Allegretto"),
        track("a.wav", "00:20", "00:30", "III. Presto"),
    ];
    match names::plan_merge(&tracks, &[2, 0, 1], &BTreeMap::new()).unwrap() {
        MergePlan::Contiguous { source, start, end, parts } => {
            assert_eq!(source, "a.wav");
            assert_eq!(names::time_seconds(&start).unwrap(), Some(0.0));
            assert_eq!(names::time_seconds(&end).unwrap(), Some(30.0));
            assert_eq!(parts.iter().map(|part| part.title.as_str()).collect::<Vec<_>>(), ["I. Adagio", "II. Allegretto", "III. Presto"]);
        }
        MergePlan::Segments { .. } => panic!("three touching spans should collapse"),
    }

    let mut near = vec![track("a.wav", "00:00", "00:01:00.000", "I"), track("a.wav", "00:01:00.040", "00:02:00", "II")];
    assert!(matches!(names::plan_merge(&near, &[0, 1], &BTreeMap::new()).unwrap(), MergePlan::Contiguous { .. }));
    near[1].start = "00:01:00.060".into();
    assert!(matches!(names::plan_merge(&near, &[0, 1], &BTreeMap::new()).unwrap(), MergePlan::Segments { .. }));

    let mut split = tracks.clone();
    split[1].source = "b.wav".into();
    match names::plan_merge(&split, &[0, 1, 2], &BTreeMap::new()).unwrap() {
        MergePlan::Segments { parts } => {
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0].source, "a.wav");
            assert_eq!(parts[1].source, "b.wav");
            assert_eq!(parts[2].source, "a.wav");
        }
        MergePlan::Contiguous { .. } => panic!("a foreign middle segment must stay separate"),
    }
}

#[test]
fn merge_fills_a_missing_end_from_the_next_span_or_the_duration() {
    let open = vec![track("a.wav", "00:00", "", "I"), track("a.wav", "00:01:30", "00:02:00", "II")];
    match names::plan_merge(&open, &[0, 1], &BTreeMap::new()).unwrap() {
        MergePlan::Contiguous { end, parts, .. } => {
            assert_eq!(names::time_seconds(&parts[0].end).unwrap(), Some(90.0));
            assert_eq!(names::time_seconds(&end).unwrap(), Some(120.0));
        }
        MergePlan::Segments { .. } => panic!("a blank end should join the following start"),
    }

    let mut other = vec![track("a.wav", "00:00", "", "I"), track("b.wav", "00:10", "00:20", "II")];
    assert!(names::plan_merge(&other, &[0, 1], &BTreeMap::new()).unwrap_err().to_string().contains("缺少终点"));
    let mut durations = BTreeMap::new();
    durations.insert("a.wav".into(), 30.0);
    match names::plan_merge(&other, &[0, 1], &durations).unwrap() {
        MergePlan::Segments { parts } => assert_eq!(names::time_seconds(&parts[0].end).unwrap(), Some(30.0)),
        MergePlan::Contiguous { .. } => panic!("different files stay segmented"),
    }
    other[1].end.clear();
    durations.insert("b.wav".into(), 5.0);
    match names::plan_merge(&other, &[0, 1], &durations).unwrap() {
        MergePlan::Segments { parts } => assert!(parts[1].end.is_empty()),
        MergePlan::Contiguous { .. } => panic!("different files stay segmented"),
    }
    let early = vec![track("a.wav", "00:20", "", "I"), track("b.wav", "00:00", "00:05", "II")];
    let mut short = BTreeMap::new();
    short.insert("a.wav".into(), 10.0);
    assert!(names::plan_merge(&early, &[0, 1], &short).unwrap_err().to_string().contains("缺少终点"));

    let tail = vec![track("a.wav", "00:00", "00:10", "I"), track("a.wav", "00:10", "", "II")];
    match names::plan_merge(&tail, &[0, 1], &BTreeMap::new()).unwrap() {
        MergePlan::Contiguous { end, .. } => assert!(end.is_empty()),
        MergePlan::Segments { .. } => panic!("the last open end can stay open"),
    }
    let mut long = durations.clone();
    long.insert("a.wav".into(), 40.0);
    match names::plan_merge(&tail, &[0, 1], &long).unwrap() {
        MergePlan::Contiguous { end, .. } => assert_eq!(names::time_seconds(&end).unwrap(), Some(40.0)),
        MergePlan::Segments { .. } => panic!("a known duration should close the last span"),
    }
}

#[test]
fn merge_expands_parts_that_were_already_joined() {
    let mut first = track("a.wav", "00:00", "00:20", "Sonata");
    first.parts = vec![
        Segment { source: "a.wav".into(), start: "00:00".into(), end: "00:10".into(), title: "I".into() },
        Segment { source: "a.wav".into(), start: "00:10".into(), end: "00:20".into(), title: "II".into() },
    ];
    let second = track("a.wav", "00:20", "00:30", "III");
    match names::plan_merge(&[first, second], &[0, 1], &BTreeMap::new()).unwrap() {
        MergePlan::Contiguous { end, parts, .. } => {
            assert_eq!(parts.len(), 3);
            assert_eq!(parts[0].title, "I");
            assert_eq!(names::time_seconds(&end).unwrap(), Some(30.0));
        }
        MergePlan::Segments { .. } => panic!("expanded parts that touch should collapse"),
    }
    let mut backwards = track("a.wav", "00:20", "00:10", "Bad");
    assert!(names::plan_merge(&[backwards.clone(), track("a.wav", "00:30", "00:40", "Next")], &[0, 1], &BTreeMap::new()).unwrap_err().to_string().contains("结束"));
    backwards.end = "00:20".into();
    backwards.start = "00:20".into();
    assert!(names::plan_merge(&[backwards, track("b.wav", "00:00", "00:05", "Next")], &[0, 1], &BTreeMap::new()).is_err());
}

#[test]
fn titles_and_movement_groups_follow_the_work_not_the_movement() {
    assert_eq!(names::shared_title(&[]), "");
    assert_eq!(names::shared_title(&["  ".into(), String::new()]), "");
    assert_eq!(names::shared_title(&["Only".into()]), "Only");
    assert_eq!(names::shared_title(&["Piano Sonata: I. Adagio".into(), "Piano Sonata: II. Allegretto".into()]), "Piano Sonata");
    assert_eq!(names::shared_title(&["Sonata in A".into(), "Sonata in B".into()]), "Sonata in");
    assert_eq!(names::shared_title(&["AB".into(), "AC".into()]), "AB");

    let sonata = titled(&["Piano Sonata: I. Adagio", "Piano Sonata: II. Allegretto", "Waltz: I. Molto", "Waltz: II. Lento"]);
    let groups = names::suggest_movement_groups(&sonata);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0], MovementGroup { indexes: vec![0, 1], title: "Piano Sonata".into() });
    assert_eq!(groups[1].title, "Waltz");

    let mut roman = titled(&["I. Allegro", "II. Adagio", "III. Presto"]);
    roman[2].source = "other.wav".into();
    let groups = names::suggest_movement_groups(&roman);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].indexes, vec![0, 1]);

    let mut discs = titled(&["Op. 27: I. Adagio", "Op. 27: II. Allegretto", "Op. 27: III. Presto"]);
    discs[2].disc = 2;
    assert_eq!(names::suggest_movement_groups(&discs)[0].indexes, vec![0, 1]);

    let broken = titled(&["Sonata: I. Adagio", "applause", "Sonata: II. Allegretto"]);
    assert!(names::suggest_movement_groups(&broken).is_empty());

    let repeated = titled(&["Work: I. A", "Work: I. B", "Work: II. C"]);
    assert_eq!(names::suggest_movement_groups(&repeated)[0].indexes, vec![1, 2]);
    assert!(names::suggest_movement_groups(&titled(&["Work: II. A", "Work: I. B"])).is_empty());

    let wide = titled(&["Suite: IX. Adagio", "Suite: X. Allegro", "Mvt. I Theme", "Mvt. IV Variation"]);
    let groups = names::suggest_movement_groups(&wide);
    assert_eq!(groups[0].indexes, vec![0, 1]);
    assert_eq!(groups[0].title, "Suite");
    assert_eq!(groups[1].indexes, vec![2, 3]);

    let marked = titled(&["月光 第九乐章", "月光 第十乐章", "练习曲 第9乐章", "练习曲 第10乐章", "夜曲：I. 柔板", "夜曲：II. 快板", "舞曲 - III. 终曲", "舞曲 - IV. 回旋"]);
    let groups = names::suggest_movement_groups(&marked);
    assert_eq!(groups.len(), 4);
    assert_eq!(groups[0].title, "月光");
    assert_eq!(groups[1].title, "练习曲");
    assert_eq!(groups[2].title, "夜曲");
    assert_eq!(groups[3].title, "舞曲");

    let mut movement = titled(&["Movement I Allegro", "Movement II Adagio"]);
    assert_eq!(names::suggest_movement_groups(&movement)[0].indexes, vec![0, 1]);
    movement[0].tags.title = "第1乐章".into();
    movement[1].tags.title = "第2乐章".into();
    movement[1].source = "other.wav".into();
    assert!(names::suggest_movement_groups(&movement).is_empty());
    let teens = names::suggest_movement_groups(&titled(&["组曲 第十一乐章", "组曲 第十二乐章"]));
    assert_eq!(teens.len(), 1);
    assert_eq!(teens[0].indexes, vec![0, 1]);
    assert_eq!(teens[0].title, "组曲");

    let impromptus = titled(&[
        "Impromptu in C minor, D. 899 No. 1",
        "Impromptu in E-flat major, D. 899 No. 2",
        "Impromptu in G-flat major, D. 899 No. 3",
        "Impromptu in A-flat major, D. 899 No. 4",
    ]);
    let groups = names::suggest_movement_groups(&impromptus);
    assert_eq!(groups.len(), 1, "{groups:?}");
    assert_eq!(groups[0].indexes, vec![0, 1, 2, 3]);
    assert!(groups[0].title.to_ascii_lowercase().contains("impromptu"), "{}", groups[0].title);

    let chinese = titled(&["舒伯特即兴曲 D.899 第1首", "舒伯特即兴曲 D.899 第2首", "舒伯特即兴曲 D.899 第3首", "舒伯特即兴曲 D.899 第4首"]);
    let groups = names::suggest_movement_groups(&chinese);
    assert_eq!(groups.len(), 1, "{groups:?}");
    assert_eq!(groups[0].indexes, vec![0, 1, 2, 3]);

    let sonatas = titled(&[
        "Piano Sonata No. 1 in F minor, Op. 2 No. 1",
        "Piano Sonata No. 2 in A major, Op. 2 No. 2",
        "Piano Sonata No. 3 in C major, Op. 2 No. 3",
    ]);
    assert!(names::suggest_movement_groups(&sonatas).is_empty(), "三首独立奏鸣曲不能因为同属 Op. 2 被合并");

    let named = titled(&["贝多芬钢奏 Op.2 第1首", "贝多芬钢奏 Op.2 第2首", "贝多芬钢奏 Op.2 第3首"]);
    assert!(names::suggest_movement_groups(&named).is_empty());

    let one = titled(&["Piano Sonata No. 1, Op. 2 No. 1: I. Allegro", "Piano Sonata No. 1, Op. 2 No. 1: II. Adagio", "Piano Sonata No. 1, Op. 2 No. 1: III. Prestissimo"]);
    let groups = names::suggest_movement_groups(&one);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].indexes, vec![0, 1, 2]);
}

#[test]
fn disc_numbers_names_and_session_tags_stay_inside_the_disc() {
    let mut project = Project::blank();
    project.disc_count = 3;
    project.naming_rule = "{Title} only".into();
    assert_eq!(names::effective_naming(&project), "{Title} only");
    project.naming_rule = names::DEFAULT_NAMING.into();
    assert_eq!(names::effective_naming(&project), "{Disc}-{Track} - {Title}");
    project.disc_count = 1;
    assert_eq!(names::effective_naming(&project), names::DEFAULT_NAMING);

    project.disc_count = 3;
    project.tracks = vec![track("a.wav", "00:00", "00:10", "A"), track("a.wav", "00:10", "00:20", "B"), track("b.wav", "00:00", "00:05", "C"), track("c.wav", "00:00", "00:05", "D")];
    project.tracks[0].disc = 1;
    project.tracks[1].disc = 1;
    project.tracks[1].tags.track = "09".into();
    project.tracks[2].disc = 2;
    project.tracks[2].tags.disc = "7".into();
    project.tracks[2].tags.total_discs = "9".into();
    project.tracks[3].disc = 0;
    assert_eq!(names::track_number(&project, &project.tracks[2], 2), 1);
    assert_eq!(names::track_number(&project, &project.tracks[3], 3), 4);
    let second = names::track_tags(&project, &project.tracks[1], 1);
    assert_eq!(second.track, "09");
    assert_eq!(second.disc, "01");
    assert_eq!(second.total_discs, "03");
    let third = names::track_tags(&project, &project.tracks[2], 2);
    assert_eq!((third.disc.as_str(), third.total_discs.as_str(), third.track.as_str()), ("7", "9", "01"));
    assert_eq!(names::filename_for("{Disc}-{TotalDiscs}-{Track}", &project.tracks[2], &third, 2), "7-9-01");
    assert_eq!(names::source_id(0, r"CD\image.flac"), "image.flac");
    assert_eq!(names::source_id(12, "image.flac"), "d12/image.flac");
    assert_eq!(names::source_leaf(r"d12\image.flac"), "image.flac");

    let mut duration = vec![
        Segment { source: "a.wav".into(), start: "00:00".into(), end: "00:10".into(), title: String::new() },
        Segment { source: "a.wav".into(), start: "00:10".into(), end: "00:30".into(), title: String::new() },
    ];
    assert_eq!(names::parts_duration(&duration), Some(30.0));
    duration[1].end.clear();
    assert_eq!(names::parts_duration(&duration), None);
    duration[1].end = "00:05".into();
    assert_eq!(names::parts_duration(&duration), None);

    project.name = "Recital".into();
    project.tracks[0].tags.composer = "Kept".into();
    let batch = names::apply_session_tags(&mut project);
    assert!(batch.matched >= 3);
    assert_eq!(project.tracks[0].tags.disc, "01");
    assert_eq!(project.tracks[0].tags.total_discs, "03");
    assert_eq!(project.tracks[0].tags.composer, "Kept");
    assert_eq!(project.tracks[2].tags.disc, "7");
    assert_eq!(project.tracks[1].tags.track, "09");
}

#[test]
fn cue_sheets_carry_disc_numbers_and_several_files() {
    let text = r#"
TITLE "Box"
PERFORMER "Pianist"
REM DISCNUMBER 4
REM DATE 1991
FILE "a.wav" WAVE
  TRACK 01 AUDIO
    TITLE "Sonata"
    INDEX 01 00:00:00
  TRACK 02 MOD
    TITLE "data"
    INDEX 01 00:01:00
FILE "b.flac" WAVE
  TRACK 03 AUDIO
    TITLE "Encore"
    INDEX 01 00:00:00
"#;
    let files = vec![("a.wav".into(), "D:/CD4/a.wav".into()), ("b.flac".into(), "D:/CD4/b.flac".into())];
    let (album, tracks) = cue::parse_cue(text, &files).unwrap();
    assert_eq!(album.disc, 4);
    assert_eq!(album.date, "1991");
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].source, "a.wav");
    assert_eq!(tracks[1].source, "b.flac");
    assert!(tracks[0].end.is_empty());
    assert_eq!(tracks[1].number, 3);

    let stem = "FILE \"image.wav\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"A\"\n    INDEX 01 00:00:00\n";
    let (album, tracks) = cue::parse_cue(stem, &[("image.flac".into(), "image.flac".into())]).unwrap();
    assert_eq!(album.disc, 0);
    assert_eq!(tracks[0].source, "image.flac");
    assert!(cue::parse_cue(stem, &[("other.flac".into(), "other.flac".into())]).is_err());
}

#[test]
fn directory_names_and_suite_titles_drop_only_the_disc_suffix() {
    assert_eq!(cue::directory_disc("CD1"), 1);
    assert_eq!(cue::directory_disc("cd01"), 1);
    assert_eq!(cue::directory_disc("Disc 3"), 3);
    assert_eq!(cue::directory_disc("disk_4"), 4);
    assert_eq!(cue::directory_disc("Disk-12"), 12);
    assert_eq!(cue::directory_disc("CD"), 0);
    assert_eq!(cue::directory_disc("encoded"), 0);
    assert_eq!(cue::directory_disc("my cd"), 0);
    assert_eq!(cue::directory_disc("cd 2 extra"), 2);

    let album = |title: &str| CueAlbum { title: title.into(), ..CueAlbum::default() };
    assert_eq!(cue::suite_title(&[album("Recital (CD 1)"), album("Recital (Disc 2)")]), "Recital");
    assert_eq!(cue::suite_title(&[album("Live in Vienna - CD 2")]), "Live in Vienna");
    assert_eq!(cue::suite_title(&[album("Pictures (Live)")]), "Pictures (Live)");
    assert_eq!(cue::suite_title(&[album("Alpha (CD1)"), album("Beta (CD2)")]), "Alpha");
    assert_eq!(cue::suite_title(&[]), "");
}

#[test]
fn collecting_a_box_pairs_cues_and_leaves_ambiguities_visible() {
    let scratch = Scratch::new();
    let write = |dir: &str, name: &str, body: &str| scratch.file(&format!("{dir}/{name}"), body.as_bytes());
    let cue = |title: &str, file: &str, disc: &str| format!("TITLE \"{title}\"\n{disc}FILE \"{file}\" WAVE\n  TRACK 01 AUDIO\n    TITLE \"Sonata\"\n    INDEX 01 00:00:00\n");
    write("CD1", "image.flac", "a");
    write("CD1", "disc.cue", &cue("Recital (CD 1)", "image.wav", ""));
    write("CD2", "image.flac", "b");
    write("CD2", "disc.cue", &cue("Recital (CD 2)", "image.flac", "REM DISCNUMBER 1\n"));
    write("CD10", "only.flac", "c");
    write("CD10", "notes.txt", "ignore");
    write("CD10", "cover.jpg", "ignore");
    write("Odd", "lost.cue", &cue("Lost", "missing.wav", "REM DISCNUMBER 8\n"));
    write("Odd", "spare.flac", "d");
    write("Pair", "one.flac", "e");
    write("Pair", "two.flac", "f");
    write("Pair", "one.cue", &cue("One", "one.flac", ""));
    write("Pair", "two.cue", &cue("Two", "two.flac", ""));

    let discs = cue::collect_discs(&cue::gather_entries(&[scratch.0.clone()]));
    let labels = discs.iter().map(|disc| format!("{}:{}:{}", disc.label, disc.number, disc.rows.len())).collect::<Vec<_>>();
    assert_eq!(discs.len(), 6, "{labels:?}");
    assert!(labels.iter().any(|label| label.contains("CD1") && label.ends_with(":1")), "{labels:?}");
    let cd1 = discs.iter().find(|disc| disc.directory.ends_with("CD1")).unwrap();
    assert_eq!(cd1.number, 1);
    assert!(cd1.issue.is_empty(), "{}", cd1.issue);
    assert_eq!(cd1.rows[0].source, "image.flac");
    let cd2 = discs.iter().find(|disc| disc.directory.ends_with("CD2")).unwrap();
    assert_eq!(cd2.number, 2, "a repeated REM DISCNUMBER is renumbered");
    assert_ne!(cd1.rows[0].path, cd2.rows[0].path);
    let loose = discs.iter().find(|disc| disc.directory.ends_with("CD10")).unwrap();
    assert!(loose.issue.contains("未配对"));
    assert!(discs.iter().any(|disc| disc.issue.contains("missing.wav") || disc.issue.contains("请导入")));
    let pair = discs.iter().filter(|disc| disc.directory.ends_with("Pair")).collect::<Vec<_>>();
    assert_eq!(pair.len(), 2);
    assert!(pair.iter().all(|disc| disc.issue.is_empty()));
    assert_eq!(cue::suite_title(&[cd1.album.clone(), cd2.album.clone()]), "Recital");
    let parsed = cue::parse_disc_cue(Path::new(&cd2.cue_path), &cd2.files).unwrap();
    assert_eq!(parsed.1.len(), 1);
    assert!(cue::parse_disc_cue(Path::new(&cd2.cue_path), &[]).is_err());
}

#[test]
fn export_plan_collapses_touches_splits_gaps_and_names_discs() {
    let scratch = Scratch::new();
    let mut project = recital(&scratch.0, &["sonata.wav", "encore.wav"]);
    project.tracks = vec![track("sonata.wav", "00:00", "00:30", "Sonata")];
    project.tracks[0].parts = vec![
        Segment { source: "sonata.wav".into(), start: "00:00".into(), end: "00:10".into(), title: "I".into() },
        Segment { source: "sonata.wav".into(), start: "00:10".into(), end: "00:30".into(), title: "II".into() },
    ];
    let plan = export::prepare(&project).unwrap();
    assert_eq!(plan.jobs[0].segments.len(), 1);
    assert_eq!(plan.jobs[0].end, Some(30.0));
    assert_eq!(plan.jobs[0].target, "01 - Sonata.flac");
    assert!(!export::describe_plan(&plan).contains("拼接"));

    project.tracks[0].parts[1].start = "00:20".into();
    let plan = export::prepare(&project).unwrap();
    assert_eq!(plan.jobs[0].segments.len(), 2);
    assert_eq!(plan.jobs[0].segments[0].end, Some(10.0));
    assert_eq!(plan.jobs[0].segments[1].start, 20.0);
    let described = export::describe_plan(&plan);
    assert!(described.contains("1 条由多段拼接"));
    assert!(described.contains("空隙不会保留"));
    assert!(described.contains("拼接 2 段"));

    project.tracks[0].parts[1].source = "encore.wav".into();
    let plan = export::prepare(&project).unwrap();
    assert_eq!(plan.jobs[0].segments.len(), 2);
    assert_ne!(plan.jobs[0].segments[0].source, plan.jobs[0].segments[1].source);

    project.tracks[0].parts[0].end.clear();
    assert!(export::prepare(&project).unwrap_err().to_string().contains("中间片段"));
    project.tracks[0].parts[0].end = "00:40".into();
    project.tracks[0].parts[0].start = "00:50".into();
    assert!(export::prepare(&project).unwrap_err().to_string().contains("结束"));

    project.tracks = vec![track("sonata.wav", "00:00", "00:10", "A"), track("encore.wav", "00:00", "00:10", "A")];
    project.tracks[0].disc = 1;
    project.tracks[1].disc = 2;
    project.disc_count = 2;
    project.disc_folders = true;
    let plan = export::prepare(&project).unwrap();
    assert_eq!(plan.jobs[0].target, "Disc 01/01-01 - A.flac");
    assert_eq!(plan.jobs[1].target, "Disc 02/02-01 - A.flac");
    assert_eq!(plan.jobs[0].tags.disc, "01");
    assert_eq!(plan.jobs[1].tags.track, "01");
    assert_eq!(plan.jobs[1].tags.total_discs, "02");
    project.disc_folders = false;
    project.naming_rule = "{Title}".into();
    assert!(export::prepare(&project).unwrap_err().to_string().contains("重复"));
    project.naming_rule.clear();
    project.tracks[1].tags.title.clear();
    assert!(export::prepare(&project).unwrap_err().to_string().contains("Title"));
    project.settings.metadata = false;
    assert!(export::prepare(&project).is_ok());

    project.source_paths.clear();
    assert!(export::prepare(&project).unwrap_err().to_string().contains("找不到"));
}

#[test]
fn source_root_and_relink_resolve_unique_leaves_only() {
    let scratch = Scratch::new();
    let audio = scratch.file("CD1/Image.FLAC", b"a");
    scratch.file("CD2/Image.FLAC", b"b");
    scratch.file("CD1/notes.txt", b"no");
    scratch.file("unique.wav", b"c");
    let mut project = Project::blank();
    project.name = "Recital".into();
    project.settings.cover = false;
    project.sources = vec!["d01/image.flac".into(), "unique.wav".into(), "missing.wav".into()];
    project.source_root = scratch.0.display().to_string();
    assert_eq!(export::relink_directory(&mut project, &scratch.0).unwrap(), 1);
    assert!(project.source_paths.get("d01/image.flac").is_none());
    let linked = export::resolve_source(&project, "unique.wav").unwrap();
    assert_eq!(linked, scratch.0.join("unique.wav"));
    assert!(export::resolve_source(&project, "d01/image.flac").is_err());

    std::fs::remove_file(scratch.0.join("CD2/Image.FLAC")).unwrap();
    assert_eq!(export::relink_directory(&mut project, &scratch.0).unwrap(), 2);
    assert_eq!(export::resolve_source(&project, "d01/image.flac").unwrap(), audio);
    assert!(export::resolve_source(&project, "missing.wav").is_err());

    project.source_paths.insert("d01/image.flac".into(), "CD1/Image.FLAC".into());
    assert_eq!(export::resolve_source(&project, "d01/image.flac").unwrap(), audio);
    project.source_root = scratch.0.join("gone").display().to_string();
    assert!(export::resolve_source(&project, "d01/image.flac").is_err());
    assert!(export::relink_directory(&mut project, &audio).unwrap_err().to_string().contains("文件夹"));
}

#[test]
fn quality_check_accepts_disc_numbers_by_format() {
    let mut made = plan("FLAC", vec![job("01.flac", 1)]);
    made.jobs[0].tags.disc = "01".into();
    made.jobs[0].tags.total_discs = "02".into();
    let mut info = media(1.0, 48000);
    info.tags.insert("title".into(), "Sonata".into());
    info.tags.insert("disc".into(), "1".into());
    info.tags.insert("totaldiscs".into(), "2".into());
    assert!(export::verify_output(&info, &made, &made.jobs[0], Some(1.0)).is_ok());
    info.tags.insert("disc".into(), "3".into());
    assert!(export::verify_output(&info, &made, &made.jobs[0], Some(1.0)).unwrap_err().to_string().contains("Disc"));
    made.format = "MP3".into();
    info.tags.remove("disc");
    assert!(export::verify_output(&info, &made, &made.jobs[0], Some(1.2)).is_ok());
    made.format = "WAV".into();
    assert!(export::verify_output(&info, &made, &made.jobs[0], Some(1.0)).is_ok());
    made.format = "FLAC".into();
    assert!(export::verify_output(&info, &made, &made.jobs[0], Some(1.31)).unwrap_err().to_string().contains("时长"));
    made.settings.cover = true;
    made.cover = Some(PathBuf::from("cover.png"));
    info.attached_pic = false;
    info.tags.insert("disc".into(), "1".into());
    assert!(export::verify_output(&info, &made, &made.jobs[0], Some(1.0)).unwrap_err().to_string().contains("封面"));

    let described = export::describe_plan(&plan("FLAC", (0..13).map(|index| job(&format!("t{index:02}"), usize::from(index == 0) + 1)).collect()));
    assert!(described.contains("将导出 13 个文件"));
    assert!(described.contains("其余 1 个文件略."));
    assert!(!described.contains("t12"));
}

#[test]
fn project_files_keep_parts_and_still_open_old_json() {
    let mut project = Project::blank();
    project.name = "Recital".into();
    project.disc_count = 2;
    project.disc_folders = true;
    project.source_root = "library".into();
    project.settings.embed_back = true;
    project.tracks[0].disc = 2;
    project.tracks[0].parts = vec![Segment { source: "a.wav".into(), start: "00:00".into(), end: "00:01".into(), title: "I".into() }];
    let text = serde_json::to_string(&project).unwrap();
    let loaded: Project = serde_json::from_str(&text).unwrap();
    assert_eq!(loaded, project);

    let scratch = Scratch::new();
    let path = scratch.0.join("project.crstudio.json");
    crate::store::write_project_file(&path, &ProjectFile::from_project(&project, &[])).unwrap();
    let read = crate::store::read_project_file(&path).unwrap();
    assert_eq!(read.project.tracks[0].parts[0].title, "I");
    assert!(read.project.settings.embed_back);
    assert!(read.project.disc_folders);

    let old = r#"{"format":"classical-recording-studio/project","version":1,"project":{"id":"p1","name":"Old","tracks":[{"id":"t","source":"a.wav","start":"00:00","end":"00:01","tags":{"Title":"Old"}}]}}"#;
    let file: ProjectFile = serde_json::from_str(old).unwrap();
    assert!(file.project.tracks[0].parts.is_empty());
    assert!(!file.project.disc_folders);
    assert!(!file.project.settings.embed_back);
    assert!(file.project.source_root.is_empty());
    assert_eq!(file.project.disc_count, 0);
}
