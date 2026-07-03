//! LLM 翻译引擎：reqwest + 手动 SSE 解析 + 批量并发。
//!
//! 架构（详见阶段调研报告）：
//! - 每批 N 条打包成 JSON 数组发给 LLM，要求返回同结构同顺序的 JSON 数组
//! - 用 id 对齐，避免模型乱序导致映射错乱
//! - Semaphore 控制并发，指数退避重试
//! - 流式 token 通过 Tauri Channel 实时推送到前端

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tokio::sync::Semaphore;

use crate::error::{AppError, Result};
use crate::types::{LlmSettings, TranslationEntry, TranslationEvent, TranslationStatus};

/// D&D / BG3 世界观语境的系统 prompt。
///
/// 关键约束：
/// 1. 保留富文本标签（<LSTag>、<font>、<i> 等）和占位符（{1}、{2}）的数量与位置
/// 2. 术语符合 D&D 5e 与 BG3 官方译名（费伦大陆、被遗忘的国度）
/// 3. 只输出译文，不加解释
pub const SYSTEM_PROMPT: &str = r#"你是一位精通《龙与地下城》第五版（D&D 5e）和《博德之门3》（Baldur's Gate 3）的专业游戏本地化译者，正在将游戏 MOD 文本从英文翻译为简体中文。

请严格遵守以下规则：

1. **世界观语境**：使用费伦大陆（Faerûn）、被遗忘的国度（Forgotten Realms）的官方译名风格。D&D 核心术语遵循官方中文译法（如 "Paladin"=圣武士、"Sorcerer"=术士、"Warlock"=邪术师、"Rogue"=游荡者、"Cleric"=牧师、"Barbarian"=野蛮人、"Wizard"=法师、"Druid"=德鲁伊、"Ranger"=游侠、"Bard"=吟游诗人、"Monk"=武僧、"Fighter"=战士）。

2. **保留占位符**：文本中的 `{1}`、`{2}` 等参数占位符必须原样保留，数量与位置不可改变。

3. **保留富文本标签**：形如 `<LSTag Tag="...">...</LSTag>`、`<font>...</font>`、`<i>...</i>` 的标签必须完整保留，只翻译标签外的自然语言文本。标签内的英文内容（如 Tag 属性值）不要翻译。

4. **语体**：贴合游戏叙事风格。法术/物品描述用典雅书面语，对话用自然口语，UI 按钮用简洁短语。保持原文的语气和正式程度。

5. **专有名词**：BG3 已有官方译名的角色、地点、物品优先用官方译法（如 "Baldur's Gate"=博德之门、"Avernus"=阿佛纳斯、"Mind Flayer"=夺心魔、"Tadpole"=蝌蚪）。

6. **只输出译文**：不要添加注释、解释、引号或前后缀。

如果输入是 JSON 数组格式，请输出相同结构、相同 id、相同顺序的 JSON 数组，不要用 ```json 代码块包裹。"#;

/// 单条翻译的请求/响应负载（JSON 数组中的元素）。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TranslateItem {
    id: String,
    text: String,
}

/// 单次批量调用的结果。
struct BatchResult {
    /// id -> 译文
    translations: std::collections::HashMap<String, String>,
    /// 失败的 id 及原因
    failed: std::collections::HashMap<String, String>,
}

/// 批量流式翻译入口。
///
/// - 只翻译 source 非空且 target 为空的条目（已翻译/已编辑的跳过）
/// - 按设置分组、并发执行
/// - 通过 channel 实时推送进度
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
    let batch_size = settings.batch_size.max(1);
    let concurrency = settings.concurrency.clamp(1, 10);
    let semaphore = Arc::new(Semaphore::new(concurrency));

    // 分组
    let batches: Vec<Vec<TranslateItem>> = to_translate
        .chunks(batch_size)
        .map(|chunk| {
            chunk
                .iter()
                .map(|e| TranslateItem {
                    id: e.id.clone(),
                    text: e.source.clone(),
                })
                .collect()
        })
        .collect();

    // 标记全部为 translating
    for item in &to_translate {
        let _ = on_event.send(TranslationEvent::Progress {
            entry_id: item.id.clone(),
            status: TranslationStatus::Translating,
        });
    }

    let url = format!(
        "{}/v1/chat/completions",
        settings.base_url.trim_end_matches('/')
    );
    let api_key = settings.api_key.clone();
    let model = settings.model.clone();

    let mut tasks = Vec::new();
    for batch in batches {
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
        tasks.push(tokio::spawn(async move {
            let _permit = permit; // 持有 permit 直到完成
            translate_batch_with_retry(&client, &url, &api_key, &model, batch, &channel).await
        }));
    }

    // 等待全部完成
    let mut failed_total = 0usize;
    for task in tasks {
        match task.await {
            Ok(Ok(result)) => {
                for (id, text) in result.translations {
                    let _ = on_event.send(TranslationEvent::Done {
                        entry_id: id,
                        text,
                    });
                }
                for (id, msg) in result.failed {
                    failed_total += 1;
                    let _ = on_event.send(TranslationEvent::Error {
                        entry_id: id,
                        message: msg,
                    });
                }
            }
            Ok(Err(e)) => {
                log::error!("批量任务失败: {e}");
                failed_total += 1;
            }
            Err(e) => {
                log::error!("任务 panic: {e}");
                failed_total += 1;
            }
        }
    }

    let _ = on_event.send(TranslationEvent::AllDone {
        total,
        failed: failed_total,
    });
    Ok(())
}

/// 单批翻译，带指数退避重试（最多 3 次）。
///
/// 当批次只有 1 条时走单条流式路径（delta 实时对应到条目，前端体验最佳）；
/// 多条时走批量 JSON 聚合路径（保上下文一致性，但无逐条流式）。
async fn translate_batch_with_retry(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    batch: Vec<TranslateItem>,
    on_event: &Channel<TranslationEvent>,
) -> Result<BatchResult> {
    let mut last_err = String::new();
    for attempt in 1..=3u32 {
        let result = if batch.len() == 1 {
            // 单条流式：delta 实时推送到该条目
            translate_single_stream(client, url, api_key, model, &batch[0], on_event)
                .await
        } else {
            translate_batch_once(client, url, api_key, model, &batch, on_event).await
        };
        match result {
            Ok(r) => return Ok(r),
            Err(e) => {
                last_err = e.to_string();
                log::warn!("第 {attempt} 次尝试失败: {last_err}");
                if attempt < 3 {
                    let backoff = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
    // 全部失败：整批标记错误
    let mut failed = std::collections::HashMap::new();
    for item in &batch {
        failed.insert(item.id.clone(), last_err.clone());
    }
    Ok(BatchResult {
        translations: std::collections::HashMap::new(),
        failed,
    })
}

/// 单条流式翻译：每个 token 通过 Delta 事件实时推送到前端对应条目。
/// 这是用户体验最好的模式（batch_size=1 时启用）。
async fn translate_single_stream(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    item: &TranslateItem,
    on_event: &Channel<TranslationEvent>,
) -> Result<BatchResult> {
    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "temperature": 0.3,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content":
                format!("请将以下文本翻译为简体中文，只输出译文，不要解释：\n\n{}", item.text)
            }
        ]
    });

    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .timeout(Duration::from_secs(120))
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
    let mut translations = std::collections::HashMap::new();

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
                    let _ = on_event.send(TranslationEvent::Delta {
                        entry_id: item.id.clone(),
                        text: delta.clone(),
                    });
                    full.push_str(&delta);
                }
            }
        }
    }

    translations.insert(item.id.clone(), full);
    Ok(BatchResult {
        translations,
        failed: std::collections::HashMap::new(),
    })
}

/// 单批单次调用。
///
/// 由于 LLM 被要求输出 JSON 数组（必须完整才能解析），流式 token 无法直接
/// 映射到具体条目。策略：
/// - 实时把 token 聚合到 full 字符串
/// - 同时把每个 token 作为"实时增量"推给本批第一个条目（让前端进度条动起来）
/// - 流结束后从 full 解析出 JSON 数组，按 id 分发真实结果，并清除占位 delta
async fn translate_batch_once(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    batch: &[TranslateItem],
    on_event: &Channel<TranslationEvent>,
) -> Result<BatchResult> {
    let _ = on_event; // 批量 JSON 模式下增量无法按 id 分发，仅用聚合结果
    let user_content = serde_json::to_string(batch)
        .map_err(|e| AppError::Llm(format!("序列化失败: {e}")))?;

    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "temperature": 0.3,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content":
                format!("请将以下 JSON 数组中的每条 text 翻译为简体中文，保持 id 和数组结构不变，只输出 JSON 数组：\n\n{user_content}")
            }
        ]
    });

    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .timeout(Duration::from_secs(120))
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

    // ── 流式聚合 ──
    // 期望的 id 映射，用于最后按 id 分发
    let mut translations = std::collections::HashMap::new();
    let mut failed = std::collections::HashMap::new();

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::Llm(format!("流读取失败: {e}")))?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // 按行切出完整 SSE event（每行一条 data:）
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
                full.push_str(&delta);
            }
        }
    }

    // ── 解析完整 JSON 数组 ──
    // 模型有时会包裹 ```json ... ```，剥离
    let json_str = strip_code_fence(&full);
    match serde_json::from_str::<Vec<TranslateItem>>(json_str) {
        Ok(items) => {
            for item in items {
                translations.insert(item.id, item.text);
            }
            // 校验：本批每个 id 都应出现
            for b in batch {
                if !translations.contains_key(&b.id) {
                    failed.insert(b.id.clone(), "模型未返回该条目".into());
                }
            }
        }
        Err(e) => {
            // JSON 解析失败：把整批标记为失败，保留原始输出便于排查
            for b in batch {
                failed.insert(b.id.clone(), format!("解析失败: {e}"));
            }
            log::warn!("批量 JSON 解析失败，原始输出: {full}");
        }
    }

    Ok(BatchResult {
        translations,
        failed,
    })
}

/// 剥离模型可能包裹的 ```json ... ``` 代码块标记。
fn strip_code_fence(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix("```json").unwrap_or(s);
    let s = s.strip_prefix("```").unwrap_or(s);
    let s = s.strip_suffix("```").unwrap_or(s);
    s.trim()
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    delta: ChatDelta,
}

#[derive(Debug, Deserialize)]
struct ChatDelta {
    #[serde(default)]
    content: Option<String>,
}
