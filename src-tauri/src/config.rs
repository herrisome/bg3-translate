//! LLM 设置的持久化。存到用户配置目录的 JSON 文件。

use std::path::PathBuf;

use crate::error::{AppError, Result};
use crate::types::LlmSettings;

/// 配置文件路径：<config_dir>/bg3-translate/settings.json
fn config_path() -> Result<PathBuf> {
    Ok(config_dir().join("settings.json"))
}

/// 应用配置目录：<config_dir>/bg3-translate/（供 glossary 等模块复用）
pub fn config_dir() -> PathBuf {
    let dir = dirs_next();
    let cfg = dir.join("bg3-translate");
    let _ = std::fs::create_dir_all(&cfg);
    cfg
}

/// macOS/Linux/Windows 各自的用户配置目录。
fn dirs_next() -> PathBuf {
    if let Ok(d) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(d);
    }
    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".config");
        }
    }
    if cfg!(target_os = "windows") {
        if let Ok(d) = std::env::var("APPDATA") {
            return PathBuf::from(d);
        }
    }
    std::env::temp_dir()
}

pub fn load() -> Result<LlmSettings> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(LlmSettings::default());
    }
    let content = std::fs::read_to_string(&path)?;
    let settings: LlmSettings =
        serde_json::from_str(&content).unwrap_or_default();
    Ok(settings)
}

pub fn save(settings: &LlmSettings) -> Result<()> {
    let path = config_path()?;
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| AppError::Config(format!("序列化失败: {e}")))?;
    std::fs::write(&path, content)?;
    Ok(())
}
