//! 本机工作区和模型配置. 不写入项目文件里的 API Key.

use std::fs;
use std::path::PathBuf;

use crate::error::{StudioError, StudioResult};
use crate::llm::LlmConfig;
use crate::model::{ProjectFile, Workspace};

pub fn app_dir() -> PathBuf {
    if cfg!(windows) {
        if let Some(dir) = std::env::var_os("APPDATA") {
            return PathBuf::from(dir).join("classical_recording_studio");
        }
    }
    if cfg!(target_os = "macos") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("classical_recording_studio");
        }
    }
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(dir).join("classical_recording_studio");
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        return PathBuf::from(home).join(".config").join("classical_recording_studio");
    }
    PathBuf::from("classical_recording_studio")
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("classical-recording-studio")
}

fn ensure(dir: &PathBuf) -> StudioResult<()> {
    fs::create_dir_all(dir)?;
    Ok(())
}

pub fn load_workspace() -> Workspace {
    let path = app_dir().join("workspace.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_workspace(workspace: &Workspace) -> StudioResult<()> {
    let dir = app_dir();
    ensure(&dir)?;
    let text = serde_json::to_string_pretty(workspace)?;
    fs::write(dir.join("workspace.json"), text)?;
    Ok(())
}

pub fn load_llm() -> LlmConfig {
    let path = app_dir().join("llm.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_llm(config: &LlmConfig) -> StudioResult<()> {
    let dir = app_dir();
    ensure(&dir)?;
    fs::write(dir.join("llm.json"), serde_json::to_string_pretty(config)?)?;
    Ok(())
}

pub fn read_project_file(path: &std::path::Path) -> StudioResult<ProjectFile> {
    let text = fs::read_to_string(path)?;
    let file: ProjectFile = serde_json::from_str(&text)?;
    if file.format != ProjectFile::FORMAT || file.version != 1 || file.project.id.trim().is_empty() {
        return Err(StudioError::new("项目文件格式或版本不受支持."));
    }
    Ok(file)
}

pub fn write_project_file(path: &std::path::Path, file: &ProjectFile) -> StudioResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(file)?)?;
    Ok(())
}
