//! 从文本节目单里读出场次和曲目. 不识别图片.

use crate::model::{Project, Track};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportedProgram {
    pub name: Option<String>,
    pub date: Option<String>,
    pub performer: Option<String>,
    pub conductor: Option<String>,
    pub composer: Option<String>,
    pub orchestra: Option<String>,
    pub city: Option<String>,
    pub venue: Option<String>,
    pub origin: Option<String>,
    pub tracks: Vec<ImportedTrack>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedTrack {
    pub title: String,
    pub composer: String,
    pub start: String,
    pub end: String,
}

pub fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    encoding_rs::GBK.decode(bytes).0.into_owned()
}

pub fn parse_program(text: &str) -> ImportedProgram {
    let mut imported = ImportedProgram::default();
    let mut in_tracks = false;
    for raw in text.split(['\n', '\r']) {
        let line = raw.trim().trim_start_matches(['•', '●', '▪']).trim();
        let line = line.trim_start_matches(['-', '–', '—']).trim();
        if line.is_empty() {
            continue;
        }
        if is_track_header(line) {
            in_tracks = true;
            continue;
        }
        if let Some((left, right)) = split_label(line) {
            if let Some(key) = field_key(left) {
                if !right.is_empty() {
                    imported.set_field(key, right);
                }
                continue;
            }
        }
        let numbered_line = numbered(line).is_some();
        let body = numbered(line).unwrap_or(line);
        let (start, end, title) = split_time(body);
        let title = title.trim().trim_start_matches(['-', '–', '—', '|', '｜']).trim();
        if title.is_empty() {
            continue;
        }
        if !numbered_line && start.is_empty() && !in_tracks {
            continue;
        }
        let (composer, title) = split_composer(title);
        if title.is_empty() {
            continue;
        }
        imported.tracks.push(ImportedTrack {
            title: clip(&title),
            composer: clip(&composer),
            start,
            end,
        });
    }
    imported
}

fn write_field(fields: &mut usize, slot: &mut String, value: &Option<String>) {
    if let Some(value) = value.as_deref().map(str::trim).filter(|item| !item.is_empty()) {
        *slot = value.to_string();
        *fields += 1;
    }
}

pub fn apply_program(project: &mut Project, imported: &ImportedProgram, include_tracks: bool) -> (usize, usize) {
    let mut fields = 0;
    write_field(&mut fields, &mut project.name, &imported.name);
    write_field(&mut fields, &mut project.date, &imported.date);
    write_field(&mut fields, &mut project.performer, &imported.performer);
    write_field(&mut fields, &mut project.conductor, &imported.conductor);
    write_field(&mut fields, &mut project.composer, &imported.composer);
    write_field(&mut fields, &mut project.orchestra, &imported.orchestra);
    write_field(&mut fields, &mut project.city, &imported.city);
    write_field(&mut fields, &mut project.venue, &imported.venue);
    write_field(&mut fields, &mut project.origin, &imported.origin);
    if !include_tracks || imported.tracks.is_empty() {
        return (fields, 0);
    }
    let source = project.sources.first().cloned().unwrap_or_default();
    project.tracks = imported
        .tracks
        .iter()
        .map(|item| {
            let mut track = Track::blank();
            track.source = source.clone();
            track.tags.title = item.title.clone();
            track.tags.composer = item.composer.clone();
            track.start = item.start.clone();
            track.end = item.end.clone();
            track
        })
        .collect();
    (fields, project.tracks.len())
}

pub fn movement_titles(text: &str) -> Vec<String> {
    text.split(|c: char| matches!(c, '|' | '｜' | '\n' | '\r' | ';' | '；'))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(clip)
        .collect()
}

pub fn expand_movements(current: &Track, titles: &[String]) -> Vec<Track> {
    let titles: Vec<&str> = titles.iter().map(|item| item.trim()).filter(|item| !item.is_empty()).collect();
    if titles.is_empty() {
        return Vec::new();
    }
    let last = titles.len() - 1;
    titles
        .into_iter()
        .enumerate()
        .map(|(index, title)| {
            let mut track = Track::blank();
            track.source = current.source.clone();
            track.disc = current.disc;
            track.tags = current.tags.clone();
            track.tags.title = title.to_string();
            track.start = if index == 0 { current.start.clone() } else { String::new() };
            track.end = if index == last { current.end.clone() } else { String::new() };
            track
        })
        .collect()
}

impl ImportedProgram {
    fn set_field(&mut self, key: &str, value: &str) {
        let value = clip(value.trim());
        let slot = match key {
            "name" => &mut self.name,
            "date" => &mut self.date,
            "performer" => &mut self.performer,
            "conductor" => &mut self.conductor,
            "composer" => &mut self.composer,
            "orchestra" => &mut self.orchestra,
            "city" => &mut self.city,
            "venue" => &mut self.venue,
            "origin" => &mut self.origin,
            _ => return,
        };
        *slot = Some(value);
    }
}

fn clip(text: &str) -> String {
    text.trim().chars().take(280).collect()
}

fn is_track_header(line: &str) -> bool {
    let line = line.trim().trim_end_matches([':', '：']).trim();
    matches!(line.to_ascii_lowercase().as_str(), "曲目" | "节目" | "节目单" | "program" | "tracks" | "tracklist")
}

fn field_key(label: &str) -> Option<&'static str> {
    const MAP: &[(&str, &[&str])] = &[
        ("name", &["场次", "专辑名", "专辑", "音乐会", "名称", "album", "album title", "concert"]),
        ("date", &["演出日期", "日期", "date", "album date"]),
        ("performer", &["演奏家", "独奏", "演出者", "performer", "soloist", "album artist"]),
        ("conductor", &["指挥家", "指挥", "conductor"]),
        ("composer", &["作曲家", "composer", "album composer"]),
        ("orchestra", &["乐团", "乐队", "orchestra"]),
        ("city", &["城市", "city"]),
        ("venue", &["音乐厅", "地点", "场馆", "venue"]),
        ("origin", &["录音来源", "来源", "origin", "source"]),
    ];
    MAP.iter().find_map(|(key, names)| names.iter().any(|name| label.eq_ignore_ascii_case(name)).then_some(*key))
}

fn split_label(line: &str) -> Option<(&str, &str)> {
    let index = line.find([':', '：'])?;
    let mark = line[index..].chars().next()?.len_utf8();
    let left = line[..index].trim();
    if left.is_empty() {
        return None;
    }
    Some((left, line[index + mark..].trim()))
}

fn numbered(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
        if i > 2 {
            return None;
        }
    }
    if i == 0 || i >= bytes.len() {
        return None;
    }
    let sep = if bytes[i] == b'.' || bytes[i] == b')' {
        1
    } else if line[i..].starts_with('、') {
        '、'.len_utf8()
    } else if line[i..].starts_with('．') {
        '．'.len_utf8()
    } else {
        return None;
    };
    let rest = line[i + sep..].trim();
    if rest.is_empty() { None } else { Some(rest) }
}

fn take_clock(text: &str) -> Option<(&str, &str)> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut groups = 0;
    loop {
        if i >= bytes.len() || !bytes[i].is_ascii_digit() {
            break;
        }
        let begin = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let width = i - begin;
        if groups == 0 && (width == 0 || width > 2) {
            return None;
        }
        if groups > 0 && width != 2 {
            return None;
        }
        groups += 1;
        if groups == 3 {
            break;
        }
        if i < bytes.len() && bytes[i] == b':' {
            i += 1;
            continue;
        }
        break;
    }
    if groups < 2 {
        return None;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        let dot = i;
        i += 1;
        let frac = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() && i - frac < 3 {
            i += 1;
        }
        if i == frac {
            i = dot;
        }
    }
    let clock = &text[..i];
    if crate::names::time_seconds(clock).ok().flatten().is_none() {
        return None;
    }
    Some((clock, &text[i..]))
}

fn strip_range_sep(text: &str) -> Option<&str> {
    let text = text.trim_start();
    for sep in ["-", "–", "—", "~", "到", "至"] {
        if let Some(rest) = text.strip_prefix(sep) {
            return Some(rest.trim_start());
        }
    }
    None
}

fn take_end_word(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    for word in ["结束", "end"] {
        if let Some(rest) = lower.strip_prefix(word) {
            if rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace()) {
                return Some(trimmed[word.len()..].trim_start());
            }
        }
    }
    None
}

fn split_time(line: &str) -> (String, String, &str) {
    let line = line.trim();
    let Some((start, rest)) = take_clock(line) else {
        return (String::new(), String::new(), line);
    };
    let rest = rest.trim_start();
    if let Some(after) = strip_range_sep(rest) {
        if let Some(title) = take_end_word(after) {
            return (start.to_string(), String::new(), title.trim());
        }
        if let Some((end, title)) = take_clock(after) {
            return (start.to_string(), end.to_string(), title.trim());
        }
    }
    (start.to_string(), String::new(), rest.trim())
}

fn split_composer(title: &str) -> (String, String) {
    let lower = title.to_ascii_lowercase();
    if let Some(index) = lower.rfind(" | composer:") {
        let name = title[..index].trim();
        let composer = title[index + " | composer:".len()..].trim();
        if !name.is_empty() && !composer.is_empty() {
            return (composer.to_string(), name.to_string());
        }
    }
    let Some((left, right)) = split_label(title) else {
        return (String::new(), title.to_string());
    };
    let width = left.chars().count();
    if right.is_empty() || width < 2 || width > 24 || field_key(left).is_some() {
        return (String::new(), title.to_string());
    }
    let folded = left.to_lowercase();
    const WORK: &[&str] = &["奏鸣曲", "交响", "协奏", "组曲", "op.", "op ", "k.", "bwv", "no."];
    if WORK.iter().any(|word| folded.contains(word)) {
        return (String::new(), title.to_string());
    }
    (left.to_string(), right.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_session_and_numbered_tracks() {
        let text = "\
场次: 钢琴独奏会
演出日期: 2026-11-06
演奏家: Michel Dalberto
指挥家: 张弦
作曲家: Mozart
乐团: 上海交响乐团
城市: 上海
音乐厅: 上海交响音乐厅
录音来源: 现场自录

曲目:
1. 莫扎特: A 大调钢琴奏鸣曲 K. 331
2. 00:12:00-00:20:10 肖邦: 船歌
";
        let parsed = parse_program(text);
        assert_eq!(parsed.name.as_deref(), Some("钢琴独奏会"));
        assert_eq!(parsed.date.as_deref(), Some("2026-11-06"));
        assert_eq!(parsed.performer.as_deref(), Some("Michel Dalberto"));
        assert_eq!(parsed.conductor.as_deref(), Some("张弦"));
        assert_eq!(parsed.composer.as_deref(), Some("Mozart"));
        assert_eq!(parsed.orchestra.as_deref(), Some("上海交响乐团"));
        assert_eq!(parsed.city.as_deref(), Some("上海"));
        assert_eq!(parsed.venue.as_deref(), Some("上海交响音乐厅"));
        assert_eq!(parsed.origin.as_deref(), Some("现场自录"));
        assert_eq!(parsed.tracks.len(), 2);
        assert_eq!(parsed.tracks[0].composer, "莫扎特");
        assert_eq!(parsed.tracks[0].title, "A 大调钢琴奏鸣曲 K. 331");
        assert!(parsed.tracks[0].start.is_empty());
        assert_eq!(parsed.tracks[1].start, "00:12:00");
        assert_eq!(parsed.tracks[1].end, "00:20:10");
        assert_eq!(parsed.tracks[1].composer, "肖邦");
        assert_eq!(parsed.tracks[1].title, "船歌");
    }

    #[test]
    fn session_apply_keeps_tracks_and_program_apply_replaces_them() {
        let parsed = parse_program("演奏家: 陈萨\n1. 月光奏鸣曲\n");
        let mut project = Project::blank();
        project.tracks[0].tags.title = "旧曲目".into();
        project.sources.push("live.flac".into());
        let (fields, tracks) = apply_program(&mut project, &parsed, false);
        assert_eq!(fields, 1);
        assert_eq!(tracks, 0);
        assert_eq!(project.performer, "陈萨");
        assert_eq!(project.tracks[0].tags.title, "旧曲目");
        let (fields, tracks) = apply_program(&mut project, &parsed, true);
        assert_eq!(fields, 1);
        assert_eq!(tracks, 1);
        assert_eq!(project.tracks.len(), 1);
        assert_eq!(project.tracks[0].tags.title, "月光奏鸣曲");
        assert_eq!(project.tracks[0].source, "live.flac");
        let parsed = parse_program("Album Title: Recital\n1. Sonata | Composer: Mozart\n");
        assert_eq!(parsed.name.as_deref(), Some("Recital"));
        assert_eq!(parsed.tracks[0].title, "Sonata");
        assert_eq!(parsed.tracks[0].composer, "Mozart");
    }

    #[test]
    fn end_word_and_clock_are_not_a_date() {
        let parsed = parse_program("12:00-结束 返场\n2026-11-06 只是一行说明\n");
        assert_eq!(parsed.tracks.len(), 1);
        assert_eq!(parsed.tracks[0].start, "12:00");
        assert!(parsed.tracks[0].end.is_empty());
        assert_eq!(parsed.tracks[0].title, "返场");
    }

    #[test]
    fn movements_keep_the_outer_times() {
        let titles = movement_titles("I. Allegro | II. Adagio；III. Rondo");
        assert_eq!(titles, vec!["I. Allegro", "II. Adagio", "III. Rondo"]);
        let mut current = Track::blank();
        current.source = "a.wav".into();
        current.start = "00:10:00".into();
        current.end = "00:40:00".into();
        current.tags.composer = "Mozart".into();
        let tracks = expand_movements(&current, &titles);
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[0].start, "00:10:00");
        assert!(tracks[0].end.is_empty());
        assert!(tracks[1].start.is_empty());
        assert!(tracks[1].end.is_empty());
        assert!(tracks[2].start.is_empty());
        assert_eq!(tracks[2].end, "00:40:00");
        assert_eq!(tracks[2].tags.composer, "Mozart");
        assert_eq!(tracks[2].source, "a.wav");
        assert_ne!(tracks[0].id, tracks[1].id);
    }

    #[test]
    fn decodes_utf8_and_gbk() {
        assert!(decode_text("演奏家: 陈萨".as_bytes()).contains("陈萨"));
        let (gbk, _, _) = encoding_rs::GBK.encode("指挥: 张弦");
        assert!(decode_text(gbk.as_ref()).contains("张弦"));
    }
}
