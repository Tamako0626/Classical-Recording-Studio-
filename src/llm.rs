//! OpenAI 兼容的 Chat Completions. Key 只留在本机配置, 不进项目文件.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{StudioError, StudioResult};
use crate::model::Project;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_base")]
    pub base_url: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub api_key: String,
}

fn default_base() -> String {
    "https://api.deepseek.com".into()
}
fn default_model() -> String {
    "deepseek-chat".into()
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            base_url: default_base(),
            model: default_model(),
            api_key: String::new(),
        }
    }
}

fn completions_url(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

fn models_url(base: &str) -> String {
    let base = base.trim().trim_end_matches('/').trim_end_matches("/chat/completions");
    format!("{base}/models")
}

pub fn chat(config: &LlmConfig, system: &str, user: &str) -> StudioResult<String> {
    if config.api_key.trim().is_empty() {
        return Err(StudioError::new("请先填写 API Key."));
    }
    if config.model.trim().is_empty() {
        return Err(StudioError::new("请先填写模型 ID."));
    }
    let body = serde_json::json!({
        "model": config.model.trim(),
        "temperature": 0.2,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": user}
        ]
    });
    let response = ureq::post(&completions_url(&config.base_url))
        .set("Authorization", &format!("Bearer {}", config.api_key.trim()))
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(|error| StudioError::new(format!("模型请求失败: {error}")))?;
    let json: Value = response.into_json().map_err(|error| StudioError::new(format!("模型返回无法解析: {error}")))?;
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| StudioError::new("模型没有返回文字."))
}

pub fn list_models(config: &LlmConfig) -> StudioResult<Vec<String>> {
    if config.api_key.trim().is_empty() {
        return Err(StudioError::new("请先填写 API Key."));
    }
    let response = ureq::get(&models_url(&config.base_url))
        .set("Authorization", &format!("Bearer {}", config.api_key.trim()))
        .set("Accept", "application/json")
        .call()
        .map_err(|error| StudioError::new(format!("无法读取模型列表: {error}")))?;
    let json: Value = response.into_json().map_err(|error| StudioError::new(error.to_string()))?;
    let mut names = json["data"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err(StudioError::new("模型列表是空的."));
    }
    Ok(names)
}

#[derive(Clone, Debug)]
pub struct EnglishSuggestion {
    pub key: String,
    pub label: String,
    pub before: String,
    pub after: String,
    pub note: String,
    pub apply: bool,
}

const FIELDS: &[(&str, &str)] = &[
    ("name", "场次 / 专辑名"),
    ("performer", "演奏家"),
    ("conductor", "指挥家"),
    ("orchestra", "乐团"),
    ("composer", "作曲家"),
    ("city", "城市"),
    ("venue", "音乐厅"),
    ("origin", "录音来源"),
];

pub fn suggest_english(config: &LlmConfig, project: &Project) -> StudioResult<Vec<EnglishSuggestion>> {
    let payload = serde_json::json!({
        "name": project.name,
        "performer": project.performer,
        "conductor": project.conductor,
        "orchestra": project.orchestra,
        "composer": project.composer,
        "city": project.city,
        "venue": project.venue,
        "origin": project.origin
    });
    let system = "You rewrite classical concert session fields into English for album tags and folder names. Keep proper names accurate. Do not invent a date. Return JSON only, with the same keys. Each value is an object {\"value\":\"...\",\"note\":\"...\"}. Leave value empty when the source is empty.";
    let raw = chat(config, system, &payload.to_string())?;
    let json = extract_json(&raw)?;
    let mut suggestions = Vec::new();
    for (key, label) in FIELDS {
        let before = field_value(project, key);
        if before.trim().is_empty() {
            continue;
        }
        let item = &json[key];
        let after = item.get("value").and_then(Value::as_str).unwrap_or("").trim().to_string();
        let note = item.get("note").and_then(Value::as_str).unwrap_or("").trim().to_string();
        if after.is_empty() && note.is_empty() {
            continue;
        }
        suggestions.push(EnglishSuggestion {
            key: (*key).into(),
            label: (*label).into(),
            before,
            after,
            note,
            apply: !after.is_empty(),
        });
    }
    Ok(suggestions)
}

fn field_value(project: &Project, key: &str) -> String {
    match key {
        "name" => project.name.clone(),
        "performer" => project.performer.clone(),
        "conductor" => project.conductor.clone(),
        "orchestra" => project.orchestra.clone(),
        "composer" => project.composer.clone(),
        "city" => project.city.clone(),
        "venue" => project.venue.clone(),
        "origin" => project.origin.clone(),
        _ => String::new(),
    }
}

pub fn apply_field(project: &mut Project, key: &str, value: &str) {
    let slot = match key {
        "name" => &mut project.name,
        "performer" => &mut project.performer,
        "conductor" => &mut project.conductor,
        "orchestra" => &mut project.orchestra,
        "composer" => &mut project.composer,
        "city" => &mut project.city,
        "venue" => &mut project.venue,
        "origin" => &mut project.origin,
        _ => return,
    };
    *slot = value.trim().to_string();
}

fn extract_json(text: &str) -> StudioResult<Value> {
    let trimmed = text.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if let Ok(value) = serde_json::from_str::<Value>(&trimmed[start..=end]) {
            return Ok(value);
        }
    }
    Err(StudioError::new("模型没有返回可解析的 JSON."))
}
