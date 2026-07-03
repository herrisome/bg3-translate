//! LLM 翻译引擎：reqwest + 手动 SSE 解析 + 逐条流式并发。
//!
//! 架构（优化后）：
//! - 每条单独请求、单独流式，token 实时通过 Channel 推送到对应条目
//! - 用 Semaphore 控制并发数（默认 concurrency 条同时翻译）
//! - 指数退避重试（每条独立，失败不影响其他条目）
//! - 丢弃旧的"批量 JSON 聚合"模式——它要求模型生成完整 JSON 数组才能解析，
//!   导致整批 N 条要全部生成完才有反馈，体验很差

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use tauri::ipc::Channel;
use tokio::sync::Semaphore;

use crate::error::{AppError, Result};
use crate::types::{LlmSettings, TranslationEntry, TranslationEvent, TranslationStatus};

/// D&D / BG3 世界观语境的系统 prompt。
pub const SYSTEM_PROMPT: &str = r#"你是一位精通《龙与地下城》第五版（D&D 5e）和《博德之门3》（Baldur's Gate 3）的专业游戏本地化译者，正在将游戏 MOD 文本从英文翻译为简体中文。

请严格遵守以下规则：

1. **世界观语境**：使用费伦大陆（Faerûn）、被遗忘的国度（Forgotten Realms）的官方译名风格。D&D 核心术语遵循官方中文译法（如 "Paladin"=圣武士、"Sorcerer"=术士、"Warlock"=邪术师、"Rogue"=游荡者、"Cleric"=牧师、"Barbarian"=野蛮人、"Wizard"=法师、"Druid"=德鲁伊、"Ranger"=游侠、"Bard"=吟游诗人、"Monk"=武僧、"Fighter"=战士）。

2. **保留占位符**：文本中的 `{1}`、`{2}` 等参数占位符必须原样保留，数量与位置不可改变。

3. **保留富文本标签**：形如 `<LSTag Tag="...">...</LSTag>`、`<font>...</font>`、`<i>...</i>` 的标签必须完整保留，只翻译标签外的自然语言文本。标签内的英文内容（如 Tag 属性值）不要翻译。

4. **语体**：贴合游戏叙事风格。法术/物品描述用典雅书面语，对话用自然口语，UI 按钮用简洁短语。保持原文的语气和正式程度。

5. **专有名词**：BG3 已有官方译名的角色、地点、物品优先用官方译法（如 "Baldur's Gate"=博德之门、"Avernus"=阿佛纳斯、"Mind Flayer"=夺心魔、"Tadpole"=蝌蚪）。

6. **只输出译文**：不要添加注释、解释、引号或前后缀。"#;

/// 批量流式翻译入口：逐条并发流式翻译。
///
/// - 只翻译 source 非空且 target 为空的条目（已翻译/已编辑的跳过）
/// - 用 Semaphore 控制并发数（同时 N 条在翻译）
/// - 通过 channel 实时推送每条的 start / delta / done / error
pub async fn translate_entries(
    client: &Client,
    settings: &LlmSettings,
    entries: &[TranslationEntry],
    on_event: &Channel<TranslationEvent>,
) -> Result<()> {
    // 筛选待翻译条目
    let to_translate: Vec<&TranslationEntry> = entries
        .iter()
        .filter(|e| !e.source.is_empty() && e.target.is_empty())
        .collect();

    if to_translate.is_empty() {
        let _ = on_event.send(TranslationEvent::AllDone {
            total: 0,
            failed: 0,
        });
        return Ok(());
    }

    let total = to_translate.len();
    let concurrency = settings.concurrency.clamp(1, 16);
    let semaphore = Arc::new(Semaphore::new(concurrency));

    let url = format!(
        "{}/v1/chat/completions",
        settings.base_url.trim_end_matches('/')
    );
    let api_key = settings.api_key.clone();
    let model = settings.model.clone();

    // 为每条创建独立的并发任务
    let mut tasks = Vec::new();
    for entry in to_translate {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| AppError::Llm(format!("并发信号量错误: {e}")))?;
        let client = client.clone();
        let url = url.clone();
        let api_key = api_key.clone();
        let model = model.clone();
        let channel = on_event.clone();
        let entry_id = entry.id.clone();
        let source = entry.source.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = permit; // 持有 permit 直到本条完成
            translate_one_with_retry(&client, &url, &api_key, &model, &entry_id, &source, &channel)
                .await
        }));
    }

    // 等待全部完成，统计失败数
    let mut failed_total = 0usize;
    for task in tasks {
        match task.await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                failed_total += 1;
                log::error!("翻译任务失败: {e}");
            }
            Err(e) => {
                failed_total += 1;
                log::error!("任务 panic: {e}");
            }
        }
    }

    let _ = on_event.send(TranslationEvent::AllDone {
        total,
        failed: failed_total,
    });
    Ok(())
}

/// 单条翻译，带指数退避重试（最多 3 次）。
async fn translate_one_with_retry(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    entry_id: &str,
    source: &str,
    on_event: &Channel<TranslationEvent>,
) -> Result<()> {
    let mut last_err = String::new();
    for attempt in 1..=3u32 {
        match translate_one_stream(client, url, api_key, model, entry_id, source, on_event).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e.to_string();
                log::warn!("[{entry_id}] 第 {attempt} 次尝试失败: {last_err}");
                if attempt < 3 {
                    let backoff = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
    // 全部失败
    let _ = on_event.send(TranslationEvent::Error {
        entry_id: entry_id.to_string(),
        message: last_err,
    });
    Ok(())
}

/// 单条流式翻译：每个 token 通过 Delta 事件实时推送到前端对应条目。
///
/// 这是唯一翻译路径——逐条独立请求，确保：
/// 1. 每条都流式显示，前端即时反馈
/// 2. 单条失败不影响其他条目
/// 3. Semaphore 控制实际并发吞吐
async fn translate_one_stream(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    entry_id: &str,
    source: &str,
    on_event: &Channel<TranslationEvent>,
) -> Result<()> {
    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "temperature": 0.3,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content":
                format!("请将以下文本翻译为简体中文，只输出译文，不要任何解释或前后缀：\n\n{source}")
            }
        ]
    });

    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .map_err(|e| AppError::Llm(format!("请求失败: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Llm(format!(
            "API 返回 {status}: {}",
            &text[..text.len().min(500)]
        )));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Llm(format!("流读取失败: {e}")))?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buf.find('\n') {
            let line: String = buf.drain(..=pos).collect();
            let line = line.trim_end_matches(['\r', '\n']);
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                continue;
            }
            let Ok(parsed) = serde_json::from_str::<ChatChunk>(data) else {
                continue;
            };
            if let Some(delta) = parsed
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.delta.content)
            {
                if !delta.is_empty() {
                    // 实时推送每个 token 到前端对应条目
                    let _ = on_event.send(TranslationEvent::Delta {
                        entry_id: entry_id.to_string(),
                        text: delta.clone(),
                    });
                    full.push_str(&delta);
                }
            }
        }
    }

    // 推送最终完整译文（前端会用它覆盖 delta 累积的结果，确保准确）
    let final_text = full.trim().to_string();
    let _ = on_event.send(TranslationEvent::Done {
        entry_id: entry_id.to_string(),
        text: final_text,
    });
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct ChatChunk {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, serde::Deserialize)]
struct ChatChoice {
    delta: ChatDelta,
}

#[derive(Debug, serde::Deserialize)]
struct ChatDelta {
    #[serde(default)]
    content: Option<String>,
}

// 保留旧的 TranslationStatus 引用，避免 unused import（types 模块导出用）
#[allow(unused_imports)]
use TranslationStatus as _UsedStatus;
