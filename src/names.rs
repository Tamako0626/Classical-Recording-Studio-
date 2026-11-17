//! 文件名, 时间点和场次文件夹.

use std::path::Path;

use std::collections::BTreeMap;

use crate::error::{StudioError, StudioResult};
use crate::model::{Project, Segment, Tags, Track, TAG_KEYS};

const JOIN_TOLERANCE: f64 = 0.05;

pub const DEFAULT_NAMING: &str = "{Track} - {Title}";

pub fn time_seconds(value: &str) -> StudioResult<Option<f64>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let (clock, frac) = value.split_once('.').unwrap_or((value, ""));
    if !frac.is_empty() && (frac.len() > 3 || !frac.chars().all(|c| c.is_ascii_digit())) {
        return Err(StudioError::new(format!("时间点无效: {value}")));
    }
    let parts: Vec<&str> = clock.split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|part| part.is_empty() || !part.chars().all(|c| c.is_ascii_digit())) {
        return Err(StudioError::new(format!("时间点无效: {value}")));
    }
    if parts.len() > 1 {
        for part in &parts[1..] {
            if part.len() != 2 || part.parse::<u32>().unwrap_or(99) > 59 {
                return Err(StudioError::new(format!("时间点无效: {value}")));
            }
        }
    }
    let mut total = 0.0;
    for part in &parts {
        total = total * 60.0 + part.parse::<f64>().unwrap_or(0.0);
    }
    if !frac.is_empty() {
        let millis = format!("{frac:0<3}").parse::<f64>().unwrap_or(0.0);
        total += millis / 1000.0;
    }
    Ok(Some(total))
}

pub fn format_seconds(seconds: f64) -> String {
    let millis = (seconds.max(0.0) * 1000.0).round() as u64;
    let hours = millis / 3_600_000;
    let minutes = (millis / 60_000) % 60;
    let secs = (millis / 1000) % 60;
    let ms = millis % 1000;
    if ms == 0 {
        format!("{hours:02}:{minutes:02}:{secs:02}")
    } else {
        format!("{hours:02}:{minutes:02}:{secs:02}.{ms:03}")
    }
}

fn reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    matches!(
        stem.to_ascii_lowercase().as_str(),
        "con" | "prn" | "aux" | "nul" | "com1" | "com2" | "com3" | "com4" | "com5" | "com6" | "com7" | "com8" | "com9" | "lpt1" | "lpt2" | "lpt3" | "lpt4" | "lpt5" | "lpt6" | "lpt7" | "lpt8" | "lpt9"
    )
}

pub fn windows_leaf(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.chars().any(|c| matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'..='\u{1f}'))
        && !name.ends_with('.')
        && !name.ends_with(' ')
        && !reserved(name)
}

pub fn sanitize_leaf(value: &str, limit: usize) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'..='\u{1f}') {
            out.push_str(" - ");
        } else {
            out.push(ch);
        }
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim().trim_end_matches(['.', ' ']);
    let mut result = trimmed.chars().take(limit).collect::<String>();
    result = result.trim().trim_end_matches(['.', ' ']).to_string();
    if reserved(&result) {
        result.push('_');
    }
    result
}

pub fn output_folder_valid(name: &str) -> bool {
    windows_leaf(name) && name.chars().count() <= 100
}

pub fn output_filename_valid(name: &str) -> bool {
    windows_leaf(name) && name.chars().count() <= 210
}

pub fn audio_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    windows_leaf(name)
        && [".wav", ".flac", ".mp3", ".mkv", ".ape", ".m4a", ".aac", ".aiff", ".aif", ".ogg", ".opus", ".wv"]
            .iter()
            .any(|ext| lower.ends_with(ext))
}

pub fn cover_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    windows_leaf(name) && (lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg"))
}

pub fn source_stem(source: &str) -> String {
    Path::new(source)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn joined_end(tracks: &[Track], index: usize, join: bool) -> String {
    let track = &tracks[index];
    if !track.end.trim().is_empty() || !join || track.source.is_empty() {
        return track.end.clone();
    }
    tracks
        .iter()
        .skip(index + 1)
        .find(|later| later.source == track.source && !later.start.trim().is_empty())
        .map(|later| later.start.clone())
        .unwrap_or_default()
}

pub fn session_tag_values(project: &Project) -> Tags {
    let composer = shared_composer(&project.composer);
    Tags {
        title: String::new(),
        artist: first_text(&[&project.performer, &project.conductor]),
        album: project.name.trim().to_string(),
        track: String::new(),
        date: first_text(&[&project.album_date, &project.date]),
        composer,
        conductor: project.conductor.trim().to_string(),
        soloist: project.performer.trim().to_string(),
        orchestra: project.orchestra.trim().to_string(),
        disc: String::new(),
        total_discs: String::new(),
    }
}

pub fn source_id(disc: u32, file_name: &str) -> String {
    let leaf = source_leaf(file_name);
    if disc == 0 {
        leaf
    } else {
        format!("d{disc:02}/{leaf}")
    }
}

pub fn source_leaf(source: &str) -> String {
    source.rsplit(['/', '\\']).next().unwrap_or(source).to_string()
}

pub fn track_number(project: &Project, track: &Track, index: usize) -> usize {
    if track.disc == 0 {
        return index + 1;
    }
    project.tracks.iter().take(index + 1).filter(|item| item.disc == track.disc).count()
}

pub fn effective_naming(project: &Project) -> String {
    let rule = project.naming_rule.trim();
    if project.disc_count > 1 && (rule.is_empty() || rule == DEFAULT_NAMING) {
        return "{Disc}-{Track} - {Title}".into();
    }
    if rule.is_empty() {
        DEFAULT_NAMING.into()
    } else {
        rule.to_string()
    }
}

pub fn parts_duration(parts: &[Segment]) -> Option<f64> {
    let mut total = 0.0;
    for part in parts {
        let start = time_seconds(&part.start).ok().flatten().unwrap_or(0.0);
        let end = time_seconds(&part.end).ok().flatten()?;
        if end <= start {
            return None;
        }
        total += end - start;
    }
    Some(total)
}

fn first_text(values: &[&str]) -> String {
    values.iter().map(|value| value.trim()).find(|value| !value.is_empty()).unwrap_or("").to_string()
}

fn shared_composer(composer: &str) -> String {
    let trimmed = composer.trim();
    if trimmed.chars().any(|ch| matches!(ch, '/' | '、' | ';' | '；')) {
        return String::new();
    }
    if trimmed.split_whitespace().any(|part| matches!(part, "&" | "＆" | "+")) {
        return String::new();
    }
    trimmed.to_string()
}

pub struct TagChange {
    pub key: &'static str,
    pub before: String,
    pub after: String,
}

pub struct SessionTagBatch {
    pub updates: Vec<(String, Vec<TagChange>)>,
    pub matched: usize,
    pub has_session: bool,
}

pub fn batch_session_tags(project: &Project) -> SessionTagBatch {
    let mut shared = session_tag_values(project);
    if shared.artist.trim().is_empty() {
        shared.artist = project.orchestra.trim().to_string();
    }
    let keys = ["Artist", "Album", "Date", "Composer", "Conductor", "Soloist", "Orchestra"];
    let has_session = keys.iter().any(|key| !shared.get(key).trim().is_empty());
    let mut updates = Vec::new();
    let mut matched = 0;
    if !has_session {
        return SessionTagBatch { updates, matched, has_session };
    }
    for (index, track) in project.tracks.iter().enumerate() {
        let blank = track.source.trim().is_empty()
            && track.filename.trim().is_empty()
            && track.end.trim().is_empty()
            && (track.start.trim().is_empty() || track.start.trim() == "00:00")
            && TAG_KEYS.iter().all(|key| track.tags.get(key).trim().is_empty());
        if blank {
            continue;
        }
        matched += 1;
        let mut fields = Vec::new();
        for key in ["Artist", "Album", "Date", "Composer", "Conductor", "Soloist", "Orchestra", "Track", "Disc", "TotalDiscs"] {
            let after = if key == "Track" {
                format!("{:02}", track_number(project, track, index))
            } else if key == "Disc" && track.disc > 0 {
                format!("{:02}", track.disc)
            } else if key == "TotalDiscs" && track.disc > 0 && project.disc_count > 0 {
                format!("{:02}", project.disc_count)
            } else {
                shared.get(key).trim().to_string()
            };
            let before = track.tags.get(key).to_string();
            if after.is_empty() || after == before.trim() {
                continue;
            }
            if matches!(key, "Track" | "Composer" | "Disc" | "TotalDiscs") && !before.trim().is_empty() {
                continue;
            }
            fields.push(TagChange { key, before, after });
        }
        if !fields.is_empty() {
            updates.push((track.id.clone(), fields));
        }
    }
    SessionTagBatch { updates, matched, has_session }
}

pub fn apply_session_tags(project: &mut Project) -> SessionTagBatch {
    let batch = batch_session_tags(project);
    for (id, fields) in &batch.updates {
        if let Some(track) = project.tracks.iter_mut().find(|track| &track.id == id) {
            for field in fields {
                track.tags.set(field.key, field.after.clone());
            }
        }
    }
    batch
}

pub fn track_tags(project: &Project, track: &Track, index: usize) -> Tags {
    let mut tags = session_tag_values(project);
    tags.track = format!("{:02}", track_number(project, track, index));
    if track.disc > 0 {
        tags.disc = format!("{:02}", track.disc);
        let total = if project.disc_count > 0 {
            project.disc_count
        } else {
            project.tracks.iter().map(|item| item.disc).max().unwrap_or(0)
        };
        if total > 0 {
            tags.total_discs = format!("{:02}", total);
        }
    }
    for key in TAG_KEYS {
        let custom = track.tags.get(key).trim();
        if !custom.is_empty() {
            tags.set(key, custom.to_string());
        }
    }
    tags
}

pub fn filename_for(rule: &str, track: &Track, tags: &Tags, index: usize) -> String {
    let stem = source_stem(&track.source);
    let custom = track.filename.trim();
    let base = if custom.is_empty() {
        let mut replaced = String::new();
        let chars: Vec<char> = rule.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '{' {
                if let Some(end) = chars[i + 1..].iter().position(|ch| *ch == '}') {
                    let key: String = chars[i + 1..i + 1 + end].iter().collect();
                    let value = if key == "Source" {
                        stem.as_str()
                    } else if key == "Order" {
                        replaced.push_str(&format!("{:02}", index + 1));
                        i += end + 2;
                        continue;
                    } else {
                        tags.get(&key)
                    };
                    replaced.push_str(value);
                    i += end + 2;
                    continue;
                }
            }
            replaced.push(chars[i]);
            i += 1;
        }
        replaced
    } else {
        source_stem(custom)
    };
    sanitize_leaf(&base, 180)
}

#[derive(Clone, Debug, PartialEq)]
pub enum MergePlan {
    Contiguous { source: String, start: String, end: String, parts: Vec<Segment> },
    Segments { parts: Vec<Segment> },
}

pub fn plan_merge(tracks: &[Track], selected: &[usize], durations: &BTreeMap<String, f64>) -> StudioResult<MergePlan> {
    let mut indexes = selected.to_vec();
    indexes.sort_unstable();
    indexes.dedup();
    if indexes.len() < 2 {
        return Err(StudioError::new("请至少选择两段乐章."));
    }
    let mut parts = Vec::new();
    for index in &indexes {
        let Some(track) = tracks.get(*index) else {
            return Err(StudioError::new("曲目选择无效."));
        };
        if track.parts.is_empty() {
            if track.source.trim().is_empty() {
                return Err(StudioError::new(format!("第 {} 轨还没有录音.", index + 1)));
            }
            parts.push(Segment {
                source: track.source.clone(),
                start: if track.start.trim().is_empty() { "00:00".into() } else { track.start.clone() },
                end: track.end.clone(),
                title: track.tags.title.clone(),
            });
        } else {
            for part in &track.parts {
                if part.source.trim().is_empty() {
                    return Err(StudioError::new(format!("第 {} 轨还有未选择的片段.", index + 1)));
                }
                parts.push(part.clone());
            }
        }
    }
    for index in 0..parts.len() {
        if !parts[index].end.trim().is_empty() {
            continue;
        }
        let start = time_seconds(&parts[index].start)?.unwrap_or(0.0);
        let filled = parts.get(index + 1).and_then(|next| {
            if next.source != parts[index].source {
                return None;
            }
            time_seconds(&next.start).ok().flatten().filter(|value| *value > start)
        });
        if let Some(end) = filled {
            parts[index].end = format_seconds(end);
            continue;
        }
        if let Some(duration) = durations.get(&parts[index].source).copied() {
            if duration > start {
                parts[index].end = format_seconds(duration);
                continue;
            }
        }
        if index + 1 != parts.len() {
            return Err(StudioError::new(format!("第 {} 段缺少终点, 时间轴无法拼接.", index + 1)));
        }
    }
    let mut timed = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let start = time_seconds(&part.start)?.unwrap_or(0.0);
        let end = time_seconds(&part.end)?;
        if let Some(end) = end {
            if end <= start {
                return Err(StudioError::new(format!("第 {} 段的结束须晚于开始.", index + 1)));
            }
        }
        timed.push((part.source.clone(), start, end));
    }
    let mut collapsed: Vec<(String, f64, Option<f64>)> = Vec::new();
    for (source, start, end) in timed {
        let join = collapsed.last().is_some_and(|last| last.0 == source && last.2.is_some_and(|last_end| (last_end - start).abs() <= JOIN_TOLERANCE));
        if join {
            collapsed.last_mut().unwrap().2 = end;
        } else {
            collapsed.push((source, start, end));
        }
    }
    if collapsed.len() == 1 {
        let (source, start, end) = &collapsed[0];
        Ok(MergePlan::Contiguous {
            source: source.clone(),
            start: format_seconds(*start),
            end: end.map(format_seconds).unwrap_or_default(),
            parts,
        })
    } else {
        Ok(MergePlan::Segments { parts })
    }
}

pub fn shared_title(titles: &[String]) -> String {
    let titles: Vec<&str> = titles.iter().map(|title| title.trim()).filter(|title| !title.is_empty()).collect();
    if titles.is_empty() {
        return String::new();
    }
    if let Some(work) = titles.iter().find_map(|title| movement_mark(title).filter(|(work, _)| !work.is_empty()).map(|(work, _)| work)) {
        if titles.iter().all(|title| movement_mark(title).is_some_and(|(other, _)| other == work || other.is_empty())) {
            return work;
        }
    }
    let mut prefix: Vec<char> = titles[0].chars().collect();
    for title in &titles[1..] {
        let chars: Vec<char> = title.chars().collect();
        let count = prefix.iter().zip(chars.iter()).take_while(|(left, right)| left == right).count();
        prefix.truncate(count);
    }
    let text: String = prefix.into_iter().collect();
    let text = text.trim().trim_end_matches([':', '：', '-', '–', '—', ',', '.', ' ']).trim();
    if text.chars().count() >= 3 {
        text.to_string()
    } else {
        titles[0].to_string()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MovementGroup {
    pub indexes: Vec<usize>,
    pub title: String,
}

pub fn suggest_movement_groups(tracks: &[Track]) -> Vec<MovementGroup> {
    let mut groups = Vec::new();
    let mut index = 0;
    while index < tracks.len() {
        let Some((work, ord)) = movement_mark(&tracks[index].tags.title) else {
            index += 1;
            continue;
        };
        let disc = tracks[index].disc;
        let source = tracks[index].source.clone();
        let mut indexes = vec![index];
        let mut previous = ord;
        let mut next = index + 1;
        while next < tracks.len() {
            if tracks[next].disc != disc {
                break;
            }
            if work.is_empty() && tracks[next].source != source {
                break;
            }
            let Some((other, order)) = movement_mark(&tracks[next].tags.title) else {
                break;
            };
            if other != work || order <= previous {
                break;
            }
            indexes.push(next);
            previous = order;
            next += 1;
        }
        if indexes.len() >= 2 {
            let title = if work.is_empty() {
                shared_title(&indexes.iter().map(|item| tracks[*item].tags.title.clone()).collect::<Vec<_>>())
            } else {
                work
            };
            groups.push(MovementGroup { indexes, title });
            index = next;
        } else {
            index += 1;
        }
    }
    groups
}

fn movement_mark(title: &str) -> Option<(String, u32)> {
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    if let Some(mark) = chinese_movement(title) {
        return Some(mark);
    }
    for sep in [": ", "：", " - ", " – ", " — "] {
        if let Some((left, right)) = title.rsplit_once(sep) {
            if let Some(ord) = leading_ord(right) {
                let work = left.trim().trim_end_matches([':', '：', '-', '–', '—']).trim();
                if !work.is_empty() {
                    return Some((work.to_string(), ord));
                }
            }
        }
    }
    leading_ord(title).map(|ord| (String::new(), ord))
}

fn chinese_movement(title: &str) -> Option<(String, u32)> {
    let start = title.find('第')?;
    let after = &title[start + '第'.len_utf8()..];
    let end = after.find("乐章")?;
    let number: String = after[..end].chars().filter(|ch| !ch.is_whitespace()).collect();
    let ord = chinese_number(&number)?;
    let work = title[..start].trim().trim_end_matches([':', '：', '-', '–', '—', ' ']).trim().to_string();
    Some((work, ord))
}

fn chinese_number(text: &str) -> Option<u32> {
    if let Ok(number) = text.parse::<u32>() {
        return (number > 0).then_some(number);
    }
    match text {
        "一" => Some(1),
        "二" => Some(2),
        "三" => Some(3),
        "四" => Some(4),
        "五" => Some(5),
        "六" => Some(6),
        "七" => Some(7),
        "八" => Some(8),
        "九" => Some(9),
        "十" => Some(10),
        _ => None,
    }
}

fn leading_ord(text: &str) -> Option<u32> {
    let mut rest = text.trim();
    let lower = rest.to_ascii_lowercase();
    for prefix in ["movement ", "mvt. ", "mvt "] {
        if lower.starts_with(prefix) {
            rest = rest[prefix.len()..].trim();
            break;
        }
    }
    let token: String = rest.chars().take_while(|ch| ch.is_ascii_alphanumeric()).collect();
    if token.is_empty() {
        return None;
    }
    if token.chars().all(|ch| ch.is_ascii_digit()) {
        let number = token.parse::<u32>().ok()?;
        return (1..=99).contains(&number).then_some(number);
    }
    roman(&token)
}

fn roman(text: &str) -> Option<u32> {
    let text = text.to_ascii_uppercase();
    if text.is_empty() || text.len() > 8 || !text.chars().all(|ch| matches!(ch, 'I' | 'V' | 'X' | 'L' | 'C')) {
        return None;
    }
    let value = |ch: char| match ch {
        'I' => 1,
        'V' => 5,
        'X' => 10,
        'L' => 50,
        'C' => 100,
        _ => 0,
    };
    let nums: Vec<i32> = text.chars().map(value).collect();
    let mut total = 0;
    let mut index = 0;
    while index < nums.len() {
        if index + 1 < nums.len() && nums[index] < nums[index + 1] {
            total += nums[index + 1] - nums[index];
            index += 2;
        } else {
            total += nums[index];
            index += 1;
        }
    }
    (total > 0).then_some(total as u32)
}

pub fn session_folder_name(project: &Project) -> String {
    let who = first_text(&[&project.performer, &project.conductor, &project.orchestra]);
    let parts = [project.date.trim(), who.as_str(), project.name.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let name = sanitize_leaf(&parts, 100);
    if output_folder_valid(&name) {
        name
    } else {
        "Album".into()
    }
}

pub fn album_folder_name(project: &Project) -> StudioResult<String> {
    let custom = project.output.trim();
    let name = if custom.is_empty() || project.auto_output {
        session_folder_name(project)
    } else {
        sanitize_leaf(custom, 100)
    };
    if output_folder_valid(&name) {
        Ok(name)
    } else {
        Err(StudioError::new("专辑文件夹名称无效."))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn parses_cue_style_and_plain_seconds() {
        assert_eq!(time_seconds("").unwrap(), None);
        assert_eq!(time_seconds("90").unwrap(), Some(90.0));
        assert_eq!(time_seconds("1:02").unwrap(), Some(62.0));
        assert_eq!(time_seconds("1:02:03.4").unwrap(), Some(3723.4));
        assert!(time_seconds("1:70").is_err());
    }

    #[test]
    fn rejects_reserved_and_builds_filename() {
        assert!(!windows_leaf("con.wav"));
        assert!(output_filename_valid("01 - Sonata.flac"));
        let project = Project::blank();
        let mut track = Track::blank();
        track.tags.title = "Sonata".into();
        let tags = track_tags(&project, &track, 0);
        assert_eq!(filename_for("{Track} - {Title}", &track, &tags, 0), "01 - Sonata");
    }

    #[test]
    fn merges_contiguous_gap_and_cross_file_movements() {
        let mut tracks = vec![Track::blank(), Track::blank(), Track::blank()];
        tracks[0].source = "sonata.wav".into();
        tracks[0].start = "00:00".into();
        tracks[0].end = "01:00".into();
        tracks[0].tags.title = "Sonata: I. Adagio".into();
        tracks[1].source = "sonata.wav".into();
        tracks[1].start = "01:00".into();
        tracks[1].end = "02:30".into();
        tracks[1].tags.title = "Sonata: II. Allegretto".into();
        tracks[2].source = "encore.wav".into();
        tracks[2].start = "00:10".into();
        tracks[2].end = "00:40".into();
        tracks[2].tags.title = "Encore".into();
        match plan_merge(&tracks, &[0, 1], &BTreeMap::new()).unwrap() {
            MergePlan::Contiguous { source, start, end, parts } => {
                assert_eq!(source, "sonata.wav");
                assert_eq!(time_seconds(&start).unwrap(), Some(0.0));
                assert_eq!(time_seconds(&end).unwrap(), Some(150.0));
                assert_eq!(parts.len(), 2);
            }
            MergePlan::Segments { .. } => panic!("contiguous movements should collapse"),
        }
        match plan_merge(&tracks, &[0, 2], &BTreeMap::new()).unwrap() {
            MergePlan::Segments { parts } => assert_eq!(parts.len(), 2),
            MergePlan::Contiguous { .. } => panic!("different files must stay segmented"),
        }
        tracks[1].start = "01:05".into();
        match plan_merge(&tracks, &[0, 1], &BTreeMap::new()).unwrap() {
            MergePlan::Segments { parts } => {
                assert_eq!(parts[0].end, "01:00");
                assert_eq!(parts[1].start, "01:05");
            }
            MergePlan::Contiguous { .. } => panic!("a gap must stay segmented"),
        }
        tracks[1].end.clear();
        assert!(plan_merge(&tracks, &[0, 1], &BTreeMap::new()).is_err());
        let mut durations = BTreeMap::new();
        durations.insert("sonata.wav".into(), 200.0);
        assert!(plan_merge(&tracks, &[0, 1], &durations).is_ok());
        tracks[0].end = "00:10".into();
        tracks[0].start = "00:20".into();
        assert!(plan_merge(&tracks, &[0, 1], &durations).unwrap_err().to_string().contains("结束"));
    }

    #[test]
    fn numbers_tracks_inside_each_disc_and_names_them() {
        let mut project = Project::blank();
        project.disc_count = 2;
        project.tracks = vec![Track::blank(), Track::blank(), Track::blank()];
        project.tracks[0].disc = 1;
        project.tracks[0].tags.title = "A".into();
        project.tracks[1].disc = 1;
        project.tracks[1].tags.title = "B".into();
        project.tracks[2].disc = 2;
        project.tracks[2].tags.title = "C".into();
        assert_eq!(track_tags(&project, &project.tracks[2], 2).track, "01");
        assert_eq!(track_tags(&project, &project.tracks[2], 2).disc, "02");
        assert_eq!(track_tags(&project, &project.tracks[1], 1).track, "02");
        assert_eq!(effective_naming(&project), "{Disc}-{Track} - {Title}");
        let tags = track_tags(&project, &project.tracks[0], 0);
        assert_eq!(filename_for(&effective_naming(&project), &project.tracks[0], &tags, 0), "01-01 - A");
        assert_eq!(source_id(2, r"CD2\image.flac"), "d02/image.flac");
        assert_eq!(source_leaf("d02/image.flac"), "image.flac");
    }

    #[test]
    fn suggests_movement_groups_from_titles() {
        let mut tracks = vec![Track::blank(), Track::blank(), Track::blank(), Track::blank()];
        tracks[0].disc = 1;
        tracks[0].tags.title = "Piano Sonata No. 14: I. Adagio".into();
        tracks[1].disc = 1;
        tracks[1].tags.title = "Piano Sonata No. 14: II. Allegretto".into();
        tracks[2].disc = 1;
        tracks[2].tags.title = "Piano Sonata No. 14: III. Presto".into();
        tracks[3].disc = 2;
        tracks[3].tags.title = "Piano Sonata No. 14: IV. Extra".into();
        let groups = suggest_movement_groups(&tracks);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].indexes, vec![0, 1, 2]);
        assert_eq!(groups[0].title, "Piano Sonata No. 14");
        tracks[0].tags.title = "月光奏鸣曲 第一乐章".into();
        tracks[1].tags.title = "月光奏鸣曲 第二乐章".into();
        tracks[2].tags.title = "Encore".into();
        let groups = suggest_movement_groups(&tracks);
        assert_eq!(groups[0].indexes, vec![0, 1]);
    }

    #[test]
    fn old_track_json_keeps_a_single_span() {
        let track: Track = serde_json::from_str(r#"{"id":"t1","source":"a.wav","start":"00:00","end":"01:00","tags":{"Title":"Sonata"}}"#).unwrap();
        assert!(track.parts.is_empty());
        assert_eq!(track.disc, 0);
        assert_eq!(track.tags.title, "Sonata");
        assert!(track.tags.disc.is_empty());
    }
}
