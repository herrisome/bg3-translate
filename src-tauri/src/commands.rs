//! Tauri 命令入口，前端通过 invoke 调用。

use reqwest::Client;
use tauri::ipc::Channel;
use tauri::State;

use crate::config;
use crate::error::{AppError, Result};
use crate::formats;
use crate::pak;
use crate::translation;
use crate::types::{
    ExtractResult, LlmSettings, PakFileKind, TranslationEntry, TranslationEvent,
};

/// 共享的应用状态。
pub struct AppState {
    pub http: Client,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            http: Client::new(),
        }
    }
}

/// 打开并解包 MOD 文件。
#[tauri::command]
pub async fn open_mod(file_path: String) -> Result<ExtractResult> {
    // 解包是 CPU/IO 密集型，放到阻塞线程池
    let result = tokio::task::spawn_blocking(move || pak::open_and_extract(&file_path))
        .await
        .map_err(|e| AppError::Other(format!("任务失败: {e}")))??;
    Ok(ExtractResult {
        work_dir: result.0,
        files: result.1,
    })
}

/// 读取指定文件的可翻译条目。
#[tauri::command]
pub async fn read_file_entries(
    work_dir: String,
    file_name: String,
) -> Result<Vec<TranslationEntry>> {
    // 需要知道文件类型：从文件名推断
    let kind = pak::classify_file(&file_name);
    let work_dir = work_dir.clone();
    let file_name = file_name.clone();
    tokio::task::spawn_blocking(move || {
        formats::read_entries(&work_dir, &file_name, &kind)
    })
    .await
    .map_err(|e| AppError::Other(format!("任务失败: {e}")))?
}

/// 写回编辑后的条目。
#[tauri::command]
pub async fn write_file_entries(
    work_dir: String,
    file_name: String,
    entries: Vec<TranslationEntry>,
) -> Result<()> {
    let kind = pak::classify_file(&file_name);
    tokio::task::spawn_blocking(move || {
        formats::write_entries(&work_dir, &file_name, &kind, &entries)
    })
    .await
    .map_err(|e| AppError::Other(format!("任务失败: {e}")))?
}

/// 重新打包。
#[tauri::command]
pub async fn repack_mod(work_dir: String, output_path: String) -> Result<()> {
    tokio::task::spawn_blocking(move || pak::repack(&work_dir, &output_path))
        .await
        .map_err(|e| AppError::Other(format!("任务失败: {e}")))?
}

/// 流式翻译条目。
#[tauri::command]
pub async fn translate_entries(
    state: State<'_, AppState>,
    settings: State<'_, std::sync::Mutex<Option<LlmSettings>>>,
    _work_dir: String,
    entries: Vec<TranslationEntry>,
    on_event: Channel<TranslationEvent>,
) -> Result<()> {
    // 取设置：优先用参数里的状态，否则从持久化配置读取
    let settings = {
        let guard = settings.lock().map_err(|e| {
            AppError::Config(format!("设置锁错误: {e}"))
        })?;
        match guard.clone() {
            Some(s) => s,
            None => config::load()?,
        }
    };

    if settings.api_key.is_empty() {
        return Err(AppError::Config(
            "未配置 API Key，请先在设置中填写大模型配置".into(),
        ));
    }

    translation::translate_entries(&state.http, &settings, &entries, &on_event).await
}

/// 保存 LLM 设置。
#[tauri::command]
pub async fn save_llm_settings(
    settings_state: State<'_, std::sync::Mutex<Option<LlmSettings>>>,
    settings: LlmSettings,
) -> Result<()> {
    config::save(&settings)?;
    let mut guard = settings_state
        .lock()
        .map_err(|e| AppError::Config(format!("设置锁错误: {e}")))?;
    *guard = Some(settings);
    Ok(())
}

/// 读取 LLM 设置。
#[tauri::command]
pub async fn load_llm_settings(
    settings_state: State<'_, std::sync::Mutex<Option<LlmSettings>>>,
) -> Result<LlmSettings> {
    // 先看内存缓存
    {
        let guard = settings_state
            .lock()
            .map_err(|e| AppError::Config(format!("设置锁错误: {e}")))?;
        if let Some(s) = guard.as_ref() {
            return Ok(s.clone());
        }
    }
    // 否则从磁盘读
    let settings = config::load()?;
    let mut guard = settings_state
        .lock()
        .map_err(|e| AppError::Config(format!("设置锁错误: {e}")))?;
    *guard = Some(settings.clone());
    Ok(settings)
}

// 避免未使用告警（PakFileKind 在 classify_file 内部用到）
#[allow(unused_imports)]
use PakFileKind as _UsedPakFileKind;
