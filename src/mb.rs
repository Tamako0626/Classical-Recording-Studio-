//! MusicBrainz 作品检索. 只读公开接口, 一次点一次请求.

use serde_json::Value;

use crate::error::{StudioError, StudioResult};

#[derive(Clone, Debug)]
pub struct WorkHit {
    pub title: String,
    pub artist: String,
}

pub fn search_works(query: &str) -> StudioResult<Vec<WorkHit>> {
    let query = query.trim();
    if query.is_empty() {
        return Err(StudioError::new("请输入曲名或作品号."));
    }
    let url = format!(
        "https://musicbrainz.org/ws/2/work?fmt=json&limit=8&query={}",
        encode(query)
    );
    let response = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(8))
        .timeout_read(std::time::Duration::from_secs(20))
        .timeout_write(std::time::Duration::from_secs(20))
        .build()
        .get(&url)
        .set("User-Agent", "ClassicalRecordingStudio/0.3 (https://github.com/Tamako0626/Classical-Recording-Studio-)")
        .set("Accept", "application/json")
        .call()
        .map_err(|error| StudioError::new(format!("MusicBrainz 请求失败: {error}")))?;
    let json: Value = response.into_json().map_err(|error| StudioError::new(error.to_string()))?;
    let hits = json["works"]
        .as_array()
        .map(|works| {
            works
                .iter()
                .filter_map(|work| {
                    let title = work.get("title").and_then(Value::as_str)?.trim().to_string();
                    if title.is_empty() {
                        return None;
                    }
                    let artist = work
                        .get("artist-credit")
                        .and_then(Value::as_array)
                        .and_then(|items| items.first())
                        .and_then(|item| item.get("name").or_else(|| item.get("artist").and_then(|artist| artist.get("name"))))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    Some(WorkHit { title, artist })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if hits.is_empty() {
        return Err(StudioError::new("没有找到匹配的作品."));
    }
    Ok(hits)
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
