//! CUE 与带轨号的曲目文字.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::{StudioError, StudioResult};

#[derive(Clone, Debug, PartialEq)]
pub struct CueAlbum {
    pub title: String,
    pub artist: String,
    pub composer: String,
    pub date: String,
}

impl Default for CueAlbum {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            composer: String::new(),
            date: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CueTrack {
    pub number: u32,
    pub source: String,
    pub path: String,
    pub start: String,
    pub end: String,
    pub title: String,
    pub artist: String,
    pub composer: String,
    pub filename: String,
}

#[derive(Clone, Debug)]
struct RawTrack {
    number: u32,
    reference: String,
    frames: Option<i64>,
    title: String,
    artist: String,
    composer: String,
}

pub fn decode_cue(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE.decode(&bytes[2..]).0.into_owned();
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE.decode(&bytes[2..]).0.into_owned();
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        if !text.contains('\u{FFFD}') {
            return text.to_string();
        }
    }
    encoding_rs::GBK.decode(bytes).0.into_owned()
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn leaf(value: &str) -> String {
    Path::new(value)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| value.trim().to_string())
}

fn frame_time(frames: i64) -> String {
    let millis = ((frames.max(0) as f64) * 1000.0 / 75.0).round() as u64;
    let hours = millis / 3_600_000;
    let minutes = (millis / 60_000) % 60;
    let secs = (millis / 1000) % 60;
    let ms = millis % 1000;
    format!("{hours:02}:{minutes:02}:{secs:02}.{ms:03}")
}

pub fn sort_cd_names(names: &mut [String]) {
    names.sort_by(|left, right| cmp_cd_name(left, right));
}

pub fn cmp_cd_name(left: &str, right: &str) -> std::cmp::Ordering {
    let mut left = left.chars().peekable();
    let mut right = right.chars().peekable();
    loop {
        match (left.peek().copied(), right.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(a), Some(b)) if a.is_ascii_digit() && b.is_ascii_digit() => {
                let mut an = 0u64;
                let mut bn = 0u64;
                while let Some(ch) = left.peek().copied() {
                    if !ch.is_ascii_digit() {
                        break;
                    }
                    an = an.saturating_mul(10).saturating_add((ch as u8 - b'0') as u64);
                    left.next();
                }
                while let Some(ch) = right.peek().copied() {
                    if !ch.is_ascii_digit() {
                        break;
                    }
                    bn = bn.saturating_mul(10).saturating_add((ch as u8 - b'0') as u64);
                    right.next();
                }
                if an != bn {
                    return an.cmp(&bn);
                }
            }
            (Some(a), Some(b)) => {
                left.next();
                right.next();
                let order = a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase());
                if order != std::cmp::Ordering::Equal {
                    return order;
                }
            }
        }
    }
}

pub fn parse_cue(text: &str, files: &[(String, String)]) -> StudioResult<(CueAlbum, Vec<CueTrack>)> {
    let mut album = CueAlbum::default();
    let mut tracks = Vec::<RawTrack>::new();
    let mut reference = String::new();
    let mut skipping = false;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some(rest) = line_key(line, "FILE") {
            let quoted = rest.trim();
            let name = if let Some(inner) = quoted.strip_prefix('"') {
                inner.split('"').next().unwrap_or("").to_string()
            } else {
                quoted.split_whitespace().next().unwrap_or("").to_string()
            };
            reference = leaf(&name);
            skipping = false;
            continue;
        }
        if let Some(rest) = line_key(line, "TRACK") {
            let mut parts = rest.split_whitespace();
            let number = parts.next().unwrap_or("").parse::<u32>().unwrap_or(0);
            let kind = parts.next().unwrap_or("").to_ascii_uppercase();
            skipping = kind != "AUDIO";
            if !skipping && number > 0 {
                tracks.push(RawTrack {
                    number,
                    reference: reference.clone(),
                    frames: None,
                    title: String::new(),
                    artist: String::new(),
                    composer: String::new(),
                });
            }
            continue;
        }
        if skipping {
            continue;
        }
        let target_is_track = !tracks.is_empty();
        if let Some(rest) = line_key(line, "TITLE") {
            assign_meta(target_is_track, &mut album, tracks.last_mut(), "title", unquote(rest));
            continue;
        }
        if let Some(rest) = line_key(line, "PERFORMER") {
            assign_meta(target_is_track, &mut album, tracks.last_mut(), "artist", unquote(rest));
            continue;
        }
        if let Some(rest) = line_key(line, "SONGWRITER") {
            assign_meta(target_is_track, &mut album, tracks.last_mut(), "composer", unquote(rest));
            continue;
        }
        if let Some(rest) = rem_key(line, "DATE") {
            assign_meta(target_is_track, &mut album, tracks.last_mut(), "date", unquote(rest));
            continue;
        }
        if let Some(rest) = rem_key(line, "COMPOSER") {
            assign_meta(target_is_track, &mut album, tracks.last_mut(), "composer", unquote(rest));
            continue;
        }
        let Some(track) = tracks.last_mut() else {
            continue;
        };
        let upper = line.to_ascii_uppercase();
        if !upper.starts_with("INDEX 01") && !upper.starts_with("INDEX\t01") {
            continue;
        }
        let stamp = line.split_whitespace().nth(2).unwrap_or("");
        let bits: Vec<&str> = stamp.split(':').collect();
        if bits.len() != 3 {
            return Err(StudioError::new(format!("第 {} 轨的 INDEX 01 时间无效.", track.number)));
        }
        let minutes = bits[1].parse::<u32>().unwrap_or(99);
        let frames = bits[2].parse::<u32>().unwrap_or(99);
        let mm = bits[0].parse::<i64>().unwrap_or(-1);
        if minutes > 59 || frames > 74 || mm < 0 {
            return Err(StudioError::new(format!("第 {} 轨的 INDEX 01 时间无效.", track.number)));
        }
        if track.frames.is_some() {
            return Err(StudioError::new(format!("第 {} 轨有多个 INDEX 01.", track.number)));
        }
        track.frames = Some((mm * 60 + minutes as i64) * 75 + frames as i64);
    }
    if tracks.is_empty() {
        return Err(StudioError::new("CUE 中没有找到 AUDIO 音轨."));
    }
    let mut numbers = std::collections::BTreeSet::new();
    let mut previous: BTreeMap<String, usize> = BTreeMap::new();
    let mut rows: Vec<CueTrack> = Vec::new();
    for track in tracks {
        if track.number == 0 || !numbers.insert(track.number) {
            return Err(StudioError::new("CUE 的轨号缺失或重复."));
        }
        let Some(frames) = track.frames else {
            return Err(StudioError::new(format!("第 {} 轨缺少 FILE 或 INDEX 01.", track.number)));
        };
        if track.reference.is_empty() {
            return Err(StudioError::new(format!("第 {} 轨缺少 FILE 或 INDEX 01.", track.number)));
        }
        let file = match_file(&track.reference, files)?;
        let key = file.0.to_lowercase();
        if let Some(prior) = previous.get(&key).copied() {
            let prior_frames = time_to_frames(&rows[prior].start);
            if frames <= prior_frames {
                return Err(StudioError::new("同一音频文件里的 INDEX 01 必须依次递增."));
            }
            rows[prior].end = frame_time(frames);
        }
        let row_index = rows.len();
        rows.push(CueTrack {
            number: track.number,
            source: file.0,
            path: file.1,
            start: frame_time(frames),
            end: String::new(),
            title: track.title,
            artist: track.artist,
            composer: track.composer,
            filename: String::new(),
        });
        previous.insert(key, row_index);
    }
    for row in &mut rows {
        if row.filename.is_empty() {
            let title = if row.title.is_empty() {
                format!("Track {:02}", row.number)
            } else {
                row.title.clone()
            };
            row.filename = crate::names::sanitize_leaf(&format!("{:02} - {title}", row.number), 180);
        }
    }
    Ok((album, rows))
}

fn time_to_frames(value: &str) -> i64 {
    let seconds = crate::names::time_seconds(value).ok().flatten().unwrap_or(0.0);
    (seconds * 75.0).round() as i64
}

fn match_file(reference: &str, files: &[(String, String)]) -> StudioResult<(String, String)> {
    let wanted = reference.to_lowercase();
    let exact: Vec<_> = files.iter().filter(|(name, _)| name.to_lowercase() == wanted).cloned().collect();
    if exact.len() == 1 {
        return Ok(exact[0].clone());
    }
    let stem = |name: &str| {
        Path::new(name)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    let wanted_stem = stem(reference);
    let loose: Vec<_> = files.iter().filter(|(name, _)| stem(name) == wanted_stem).cloned().collect();
    if loose.len() == 1 {
        return Ok(loose[0].clone());
    }
    Err(StudioError::new(format!("请导入 CUE 引用的音频文件: {reference}.")))
}

fn line_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let upper = line.to_ascii_uppercase();
    let prefix = format!("{key} ");
    if upper.starts_with(&prefix) {
        Some(&line[key.len() + 1..])
    } else {
        None
    }
}

fn rem_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let upper = line.to_ascii_uppercase();
    let prefix = format!("REM {key} ");
    if upper.starts_with(&prefix) {
        Some(&line[prefix.len()..])
    } else {
        None
    }
}

fn assign_meta(is_track: bool, album: &mut CueAlbum, track: Option<&mut RawTrack>, key: &str, value: String) {
    if value.trim().is_empty() {
        return;
    }
    let value = value.trim().to_string();
    if is_track {
        if let Some(track) = track {
            match key {
                "title" => track.title = value,
                "artist" => track.artist = value,
                "composer" => track.composer = value,
                _ => {}
            }
        }
    } else {
        match key {
            "title" => album.title = value,
            "artist" => album.artist = value,
            "composer" => album.composer = value,
            "date" => album.date = value,
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ListedTracks {
    pub album: CueAlbum,
    pub numbered: BTreeMap<u32, (String, String)>,
    pub ordered: Vec<(String, String)>,
    pub issues: Vec<String>,
}

pub fn parse_track_list(text: &str) -> ListedTracks {
    let mut listed = ListedTracks::default();
    let mut saw_track = false;
    for raw in text.lines() {
        let line = nfkc_digits(&strip_markup(raw));
        if line.is_empty() {
            continue;
        }
        if let Some((number, title)) = numbered_line(&line) {
            saw_track = true;
            if number == 0 || listed.numbered.contains_key(&number) {
                listed.issues.push(format!("轨号 {number} 重复或无效, 请核对文字."));
                continue;
            }
            let (title, composer) = split_composer(&strip_duration(&title));
            if title.is_empty() {
                continue;
            }
            listed.numbered.insert(number, (title.clone(), composer.clone()));
            listed.ordered.push((title, composer));
            continue;
        }
        let Some((label, value)) = line.split_once([':', '：']) else {
            continue;
        };
        let label = label.trim();
        if saw_track && !album_label_after_tracks(label) {
            continue;
        }
        let value = value.trim().trim_matches(['"', '“', '”']).trim();
        if value.is_empty() || unknown_album_value(value) {
            continue;
        }
        let value = nfkc_digits(value);
        match album_key(label) {
            "title" if listed.album.title.is_empty() => listed.album.title = value,
            "artist" if listed.album.artist.is_empty() => listed.album.artist = value,
            "composer" if listed.album.composer.is_empty() => listed.album.composer = value,
            "date" if listed.album.date.is_empty() => listed.album.date = album_date(&value),
            _ => {}
        }
    }
    listed
}

fn strip_markup(raw: &str) -> String {
    raw.trim().trim_start_matches(['-', '*', '•', '#']).trim().replace("**", "").replace("__", "")
}

fn nfkc_digits(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\u{ff10}'..='\u{ff19}' => char::from_u32(ch as u32 - 0xff10 + '0' as u32).unwrap_or(ch),
            '\u{ff21}'..='\u{ff3a}' => char::from_u32(ch as u32 - 0xff21 + 'A' as u32).unwrap_or(ch),
            '\u{ff41}'..='\u{ff5a}' => char::from_u32(ch as u32 - 0xff41 + 'a' as u32).unwrap_or(ch),
            _ => ch,
        })
        .collect()
}

fn unknown_album_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "unknown" | "n/a" | "na" | "not specified" | "not provided" | "未知" | "未提供" | "不详" | "—" | "-"
    )
}

fn album_label_after_tracks(label: &str) -> bool {
    let lower = label.to_ascii_lowercase();
    lower.starts_with("album ")
        || lower.starts_with("release ")
        || lower.starts_with("recording ")
        || label.starts_with("专辑")
        || label.starts_with("唱片")
        || label.starts_with("发行")
        || label.starts_with("录音")
}

fn album_key(label: &str) -> &'static str {
    let lower = label.to_ascii_lowercase();
    if lower.contains("composer") || label.contains("作曲") {
        "composer"
    } else if lower.contains("artist") || lower.contains("performer") || label.contains("演出") || label.contains("演奏") || label.contains("艺术家") {
        "artist"
    } else if lower.contains("date") || lower.contains("year") || label.contains("日期") || label.contains("年份") || label.contains("发行年") {
        "date"
    } else if lower.contains("album") || lower.contains("title") || lower.contains("release") || label.contains("专辑") || label.contains("唱片") {
        "title"
    } else {
        ""
    }
}

fn album_date(value: &str) -> String {
    let value = value.trim();
    if let Some(head) = value.get(..10) {
        let bytes = head.as_bytes();
        if bytes[4] == b'-' && bytes[7] == b'-' && bytes.iter().enumerate().all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit()) {
            return head.to_string();
        }
    }
    let year: String = value.chars().take(4).collect();
    if year.len() == 4 && year.chars().all(|ch| ch.is_ascii_digit()) && value.chars().nth(4).is_none_or(|ch| !ch.is_ascii_digit()) {
        return year;
    }
    value.to_string()
}

fn strip_duration(title: &str) -> String {
    let trimmed = title.trim();
    let end = trimmed.strip_suffix(')').unwrap_or(trimmed);
    let Some(space) = end.rfind(char::is_whitespace) else {
        return trimmed.to_string();
    };
    let tail = end[space + 1..].trim_start_matches('(');
    if clock_tail(tail) {
        return end[..space].trim().to_string();
    }
    trimmed.to_string()
}

fn clock_tail(text: &str) -> bool {
    let mut parts = text.split(':');
    let Some(minutes) = parts.next() else {
        return false;
    };
    let Some(seconds) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && (1..=3).contains(&minutes.len())
        && minutes.chars().all(|ch| ch.is_ascii_digit())
        && seconds.len() == 2
        && seconds.chars().all(|ch| ch.is_ascii_digit())
        && seconds.parse::<u32>().unwrap_or(99) <= 59
}

fn numbered_line(line: &str) -> Option<(u32, String)> {
    let rest = line.trim_start_matches("track").trim_start_matches("Track").trim_start_matches("TRACK").trim();
    let mut digits = String::new();
    let mut chars = rest.chars().peekable();
    while let Some(ch) = chars.peek().copied() {
        if ch.is_ascii_digit() && digits.len() < 2 {
            digits.push(ch);
            chars.next();
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let number = digits.parse::<u32>().ok()?;
    if number == 0 {
        return None;
    }
    let mark = chars.peek().copied()?;
    if !(mark.is_whitespace() || matches!(mark, '.' | '、' | ')' | '：' | ':' | '-' | '–' | '—')) {
        return None;
    }
    while let Some(ch) = chars.peek().copied() {
        if ch.is_whitespace() || matches!(ch, '.' | '、' | ')' | '：' | ':' | '-' | '–' | '—') {
            chars.next();
        } else {
            break;
        }
    }
    let title = chars.collect::<String>().trim().to_string();
    if title.is_empty() {
        None
    } else {
        Some((number, title))
    }
}

fn split_composer(title: &str) -> (String, String) {
    let lower = title.to_ascii_lowercase();
    if let Some(index) = lower.rfind(" | composer:") {
        let name = title[..index].trim();
        let composer = title[index + " | composer:".len()..].trim();
        if !name.is_empty() && !composer.is_empty() {
            return (name.to_string(), composer.to_string());
        }
    }
    if let Some((name, composer)) = title.rsplit_once(" / ") {
        return (name.trim().to_string(), composer.trim().to_string());
    }
    (title.trim().to_string(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_cue_sheet() {
        let text = r#"
TITLE "Recital"
PERFORMER "Pianist"
FILE "disc.wav" WAVE
  TRACK 01 AUDIO
    TITLE "Sonata"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "Encore"
    INDEX 01 03:00:00
"#;
        let files = vec![("disc.wav".into(), "C:/music/disc.wav".into())];
        let (album, tracks) = parse_cue(text, &files).unwrap();
        assert_eq!(album.title, "Recital");
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].end, tracks[1].start);
        assert!(tracks[1].end.is_empty());
    }
}
