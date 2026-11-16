//! 文件名, 时间点和场次文件夹.

use std::path::Path;

use crate::error::{StudioError, StudioResult};
use crate::model::{Project, Tags, Track, TAG_KEYS};

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
    let composer = if project.composer.chars().any(|ch| matches!(ch, '/' | '、' | ';' | '；' | '&' | '＆' | '+')) {
        String::new()
    } else {
        project.composer.trim().to_string()
    };
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
    }
}

fn first_text(values: &[&str]) -> String {
    values.iter().map(|value| value.trim()).find(|value| !value.is_empty()).unwrap_or("").to_string()
}

pub fn track_tags(project: &Project, track: &Track, index: usize) -> Tags {
    let mut tags = session_tag_values(project);
    tags.track = format!("{:02}", index + 1);
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
}
