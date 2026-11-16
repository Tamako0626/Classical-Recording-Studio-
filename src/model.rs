//! 工作区, 项目和曲目. 字段名与旧的 `.crstudio.json` 对齐.

use serde::{Deserialize, Serialize};

use crate::names::DEFAULT_NAMING;

pub const STATUSES: [&str; 9] = [
    "待整理",
    "待核对时间点",
    "已分轨",
    "已重命名",
    "已嵌入封面",
    "已增益",
    "已完成",
    "处理失败",
    "需修复",
];

pub const TAG_KEYS: [&str; 9] = [
    "Title",
    "Artist",
    "Album",
    "Track",
    "Date",
    "Composer",
    "Conductor",
    "Soloist",
    "Orchestra",
];

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn default_format() -> String {
    "FLAC".into()
}
fn default_rate() -> u32 {
    48000
}
fn default_width() -> f64 {
    100.0
}
fn default_true() -> bool {
    true
}
fn default_status() -> String {
    "待整理".into()
}
fn default_output() -> String {
    "Processed".into()
}
fn default_cover_status() -> String {
    "待准备".into()
}
fn off() -> String {
    "off".into()
}
fn source_channels() -> String {
    "source".into()
}
fn eight() -> f64 {
    8.0
}
fn neg_eighteen() -> f64 {
    -18.0
}
fn one() -> f64 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effects {
    #[serde(default = "off")]
    pub denoise: String,
    #[serde(default = "eight")]
    pub noise_reduction: f64,
    #[serde(default = "off")]
    pub hum: String,
    #[serde(default)]
    pub declick: bool,
    #[serde(default)]
    pub declip: bool,
    #[serde(default)]
    pub highpass: f64,
    #[serde(default)]
    pub lowpass: f64,
    #[serde(default)]
    pub bass: f64,
    #[serde(default)]
    pub mid: f64,
    #[serde(default)]
    pub treble: f64,
    #[serde(default = "source_channels")]
    pub channels: String,
    #[serde(default)]
    pub gate: f64,
    #[serde(default = "one")]
    pub compress: f64,
    #[serde(default = "neg_eighteen")]
    pub threshold: f64,
    #[serde(default)]
    pub deess: bool,
    #[serde(default = "off")]
    pub normalize: String,
    #[serde(default = "neg_eighteen")]
    pub lufs: f64,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default)]
    pub limiter: f64,
}

impl Default for Effects {
    fn default() -> Self {
        Self {
            denoise: off(),
            noise_reduction: 8.0,
            hum: off(),
            declick: false,
            declip: false,
            highpass: 0.0,
            lowpass: 0.0,
            bass: 0.0,
            mid: 0.0,
            treble: 0.0,
            channels: source_channels(),
            gate: 0.0,
            compress: 1.0,
            threshold: -18.0,
            deess: false,
            normalize: off(),
            lufs: -18.0,
            fade_in: 0.0,
            fade_out: 0.0,
            limiter: 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_rate")]
    pub rate: u32,
    #[serde(default)]
    pub gain: f64,
    #[serde(default = "default_width")]
    pub width: f64,
    #[serde(default)]
    pub reverb: f64,
    #[serde(default = "default_true")]
    pub cover: bool,
    #[serde(default = "default_true")]
    pub split: bool,
    #[serde(default = "default_true")]
    pub metadata: bool,
    #[serde(default)]
    pub effects: Effects,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: default_format(),
            rate: default_rate(),
            gain: 0.0,
            width: 100.0,
            reverb: 0.0,
            cover: true,
            split: true,
            metadata: true,
            effects: Effects::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tags {
    #[serde(default, rename = "Title")]
    pub title: String,
    #[serde(default, rename = "Artist")]
    pub artist: String,
    #[serde(default, rename = "Album")]
    pub album: String,
    #[serde(default, rename = "Track")]
    pub track: String,
    #[serde(default, rename = "Date")]
    pub date: String,
    #[serde(default, rename = "Composer")]
    pub composer: String,
    #[serde(default, rename = "Conductor")]
    pub conductor: String,
    #[serde(default, rename = "Soloist")]
    pub soloist: String,
    #[serde(default, rename = "Orchestra")]
    pub orchestra: String,
}

impl Default for Tags {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            track: String::new(),
            date: String::new(),
            composer: String::new(),
            conductor: String::new(),
            soloist: String::new(),
            orchestra: String::new(),
        }
    }
}

impl Tags {
    pub fn get(&self, key: &str) -> &str {
        match key {
            "Title" => &self.title,
            "Artist" => &self.artist,
            "Album" => &self.album,
            "Track" => &self.track,
            "Date" => &self.date,
            "Composer" => &self.composer,
            "Conductor" => &self.conductor,
            "Soloist" => &self.soloist,
            "Orchestra" => &self.orchestra,
            _ => "",
        }
    }

    pub fn set(&mut self, key: &str, value: String) {
        let slot = match key {
            "Title" => &mut self.title,
            "Artist" => &mut self.artist,
            "Album" => &mut self.album,
            "Track" => &mut self.track,
            "Date" => &mut self.date,
            "Composer" => &mut self.composer,
            "Conductor" => &mut self.conductor,
            "Soloist" => &mut self.soloist,
            "Orchestra" => &mut self.orchestra,
            _ => return,
        };
        *slot = value;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub end: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub tags: Tags,
}

impl Track {
    pub fn blank() -> Self {
        Self {
            id: new_id(),
            source: String::new(),
            start: "00:00".into(),
            end: String::new(),
            filename: String::new(),
            tags: Tags::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub performer: String,
    #[serde(default)]
    pub conductor: String,
    #[serde(default)]
    pub orchestra: String,
    #[serde(default)]
    pub composer: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub album_date: String,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub venue: String,
    #[serde(default)]
    pub origin: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub cover_file: String,
    #[serde(default)]
    pub cover_path: String,
    #[serde(default = "default_cover_status")]
    pub cover_status: String,
    #[serde(default)]
    pub back_cover_file: String,
    #[serde(default)]
    pub back_cover_path: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub source_paths: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub notes: String,
    #[serde(default = "default_output")]
    pub output: String,
    #[serde(default)]
    pub auto_output: bool,
    #[serde(default)]
    pub auto_join: bool,
    #[serde(default)]
    pub naming_rule: String,
    #[serde(default)]
    pub settings: Settings,
}

impl Project {
    pub fn blank() -> Self {
        Self {
            id: new_id(),
            name: String::new(),
            performer: String::new(),
            conductor: String::new(),
            orchestra: String::new(),
            composer: String::new(),
            date: String::new(),
            album_date: String::new(),
            city: String::new(),
            venue: String::new(),
            origin: String::new(),
            status: default_status(),
            cover_file: String::new(),
            cover_path: String::new(),
            cover_status: default_cover_status(),
            back_cover_file: String::new(),
            back_cover_path: String::new(),
            sources: Vec::new(),
            source_paths: std::collections::BTreeMap::new(),
            tracks: vec![Track::blank()],
            notes: String::new(),
            output: String::new(),
            auto_output: true,
            auto_join: false,
            naming_rule: DEFAULT_NAMING.into(),
            settings: Settings::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub project_id: String,
    #[serde(default)]
    pub project_name: String,
    pub time: String,
    pub operation: String,
    pub result: String,
    pub message: String,
}

impl LogEntry {
    pub fn new(project: &Project, operation: &str, result: &str, message: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            time: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
            operation: operation.into(),
            result: result.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub rule: String,
    pub output: String,
    pub settings: Settings,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(default = "default_naming_rule")]
    pub naming: String,
    #[serde(default)]
    pub projects: Vec<Project>,
    #[serde(default)]
    pub logs: Vec<LogEntry>,
    #[serde(default = "default_templates")]
    pub templates: Vec<Template>,
}

fn default_naming_rule() -> String {
    DEFAULT_NAMING.into()
}

fn default_templates() -> Vec<Template> {
    let naming: String = DEFAULT_NAMING.into();
    vec![
        Template {
            id: "archive-flac".into(),
            name: "分轨与专辑归档".into(),
            rule: naming.clone(),
            output: "Processed".into(),
            settings: Settings::default(),
        },
        Template {
            id: "flac-cover".into(),
            name: "整轨转 FLAC 并嵌入封面".into(),
            rule: "{Source}".into(),
            output: "FLAC Cover".into(),
            settings: Settings {
                split: false,
                ..Settings::default()
            },
        },
        Template {
            id: "gain-width".into(),
            name: "增益与声场处理".into(),
            rule: "{Source}".into(),
            output: "Audio Processing".into(),
            settings: Settings {
                split: false,
                cover: false,
                metadata: false,
                gain: 16.0,
                width: 130.0,
                ..Settings::default()
            },
        },
    ]
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            naming: default_naming_rule(),
            projects: Vec::new(),
            logs: Vec::new(),
            templates: default_templates(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectFile {
    pub format: String,
    pub version: u32,
    pub project: Project,
    #[serde(default)]
    pub logs: Vec<LogEntry>,
}

impl ProjectFile {
    pub const FORMAT: &'static str = "classical-recording-studio/project";

    pub fn from_project(project: &Project, logs: &[LogEntry]) -> Self {
        Self {
            format: Self::FORMAT.into(),
            version: 1,
            project: project.clone(),
            logs: logs
                .iter()
                .filter(|log| log.project_id == project.id)
                .cloned()
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectImport {
    Existing,
    Copy,
    New,
}

pub fn import_project(workspace: &mut Workspace, file: ProjectFile) -> ProjectImport {
    if let Some(index) = workspace.projects.iter().position(|project| project.id == file.project.id) {
        if workspace.projects[index] == file.project {
            return ProjectImport::Existing;
        }
        let mut imported = file.project;
        imported.id = new_id();
        workspace.projects.push(imported);
        return ProjectImport::Copy;
    }
    workspace.logs.extend(file.logs);
    workspace.projects.push(file.project);
    ProjectImport::New
}
