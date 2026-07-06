//! LLM 翻译引擎：reqwest + 手动 SSE 解析 + 逐条流式并发。
//!
//! 架构（优化后）：
//! - 每条单独请求、单独流式，token 实时通过 Channel 推送到对应条目
//! - 用 Semaphore 控制并发数（默认 concurrency 条同时翻译）
//! - 指数退避重试（每条独立，失败不影响其他条目）
//! - 丢弃旧的"批量 JSON 聚合"模式——它要求模型生成完整 JSON 数组才能解析，
//!   导致整批 N 条要全部生成完才有反馈，体验很差

use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use tauri::ipc::Channel;
use tokio::sync::Semaphore;

use crate::error::{AppError, Result};
use crate::types::{LlmSettings, TranslationEntry, TranslationEvent, TranslationStatus};

#[derive(Debug, Clone)]
struct ConsistencyTerm {
    source: String,
    target: String,
}

#[derive(Debug, Clone)]
struct SeriesVariant {
    base: String,
    suffix: String,
}

#[derive(Debug, Clone)]
struct SeriesMember {
    entry_id: String,
    suffix: String,
}

#[derive(Debug, Clone)]
enum TranslationOutput {
    Single { entry_id: String },
    ExactGroup { entry_ids: Vec<String> },
    Series { members: Vec<SeriesMember> },
}

#[derive(Debug, Clone)]
struct TranslationJob {
    source: String,
    matches: Vec<crate::glossary::MatchedTerm>,
    consistency_terms: Vec<ConsistencyTerm>,
    output: TranslationOutput,
}

#[derive(Debug)]
struct PendingSeriesGroup {
    base_source: String,
    members: Vec<SeriesMember>,
}

#[derive(Debug, Default)]
struct SeriesAliases {
    aliases: HashMap<String, String>,
    canonical_sources: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct SeriesBaseProfile {
    source: String,
    suffixes: HashSet<String>,
    has_latin: bool,
    has_cjk: bool,
}

impl SeriesAliases {
    fn canonical_key(&self, key: &str) -> String {
        self.aliases
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    fn source_for(&self, key: &str, fallback: &str) -> String {
        self.canonical_sources
            .get(key)
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }
}

impl TranslationJob {
    fn target_ids(&self) -> Vec<String> {
        match &self.output {
            TranslationOutput::Single { entry_id } => vec![entry_id.clone()],
            TranslationOutput::ExactGroup { entry_ids } => entry_ids.clone(),
            TranslationOutput::Series { members } => {
                members.iter().map(|m| m.entry_id.clone()).collect()
            }
        }
    }
}

fn is_cancelled(cancel_flag: &Arc<AtomicBool>) -> bool {
    cancel_flag.load(Ordering::SeqCst)
}

async fn wait_until_cancelled(cancel_flag: Arc<AtomicBool>) {
    while !is_cancelled(&cancel_flag) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// D&D / BG3 世界观语境的系统 prompt。
///
/// 注意：具体术语译名不再硬编码于此，而是通过术语表动态注入。
/// 见 build_user_prompt() 中命中的术语会被作为参考附在 user message。
pub const SYSTEM_PROMPT: &str = r#"你是一位精通《龙与地下城》第五版（D&D 5e）和《博德之门3》（Baldur's Gate 3）的专业游戏本地化译者，正在将游戏 MOD 文本从英文翻译为简体中文。

请严格遵守以下规则：

1. **世界观语境**：使用费伦大陆（Faerûn）、被遗忘的国度（Forgotten Realms）的官方译名风格。

2. **严格遵循术语表**：如果文本下方附有【术语参考】，必须严格使用其中给出的官方译名，不可自行更改。如 "Paladin"=圣武士（非"圣骑士"）、"Warlock"=邪术师（非"术士"）、"Rogue"=游荡者（非"盗贼"）。

3. **保留占位符**：文本中的 `{1}`、`{2}` 等参数占位符必须原样保留，数量与位置不可改变。

4. **保留富文本标签**：形如 `<LSTag Tag="...">...</LSTag>`、`<font>...</font>`、`<i>...</i>` 的标签必须完整保留，只翻译标签外的自然语言文本。标签内的英文内容（如 Tag 属性值）不要翻译。

5. **语体**：贴合游戏叙事风格。法术/物品描述用典雅书面语，对话用自然口语，UI 按钮用简洁短语。保持原文的语气和正式程度。

6. **同 MOD 一致性**：同一 MOD 内相同名称、相同专有名词、同系列编号/变体必须使用同一中文译名。编号、字母后缀和占位符应原样保留。

7. **只输出译文**：不要添加注释、解释、引号或前后缀。"#;

/// 构造 user prompt：注入命中的术语与本 MOD 内已确定译名作为参考。
fn build_user_prompt(
    source: &str,
    matches: &[crate::glossary::MatchedTerm],
    consistency_terms: &[ConsistencyTerm],
    style_hint: Option<&str>,
) -> String {
    let mut sections = Vec::new();
    if let Some(style_hint) = style_hint.and_then(normalize_style_hint) {
        sections.push(format!(
            "【本 MOD 翻译语境】（用于判断词义、名词类型和语体；不得覆盖术语表、标签、占位符与一致性规则）：\n{style_hint}"
        ));
    }
    if !matches.is_empty() {
        let terms: Vec<String> = matches
            .iter()
            .map(|m| format!("{} = {}", m.source, m.target))
            .collect();
        sections.push(format!(
            "【术语参考】（严格使用以下官方译名，不可更改）：\n{}",
            terms.join("\n")
        ));
    }
    if !consistency_terms.is_empty() {
        let terms: Vec<String> = consistency_terms
            .iter()
            .map(|m| format!("{} = {}", m.source, m.target))
            .collect();
        sections.push(format!(
            "【本 MOD 已确定译名】（必须保持一致，不可改写同一名称的译法）：\n{}",
            terms.join("\n")
        ));
    }

    if sections.is_empty() {
        format!("请将以下文本翻译为简体中文，只输出译文，不要任何解释或前后缀：\n\n{source}")
    } else {
        format!(
            "请将以下文本翻译为简体中文，只输出译文，不要任何解释或前后缀。\n\n{}\n\n原文：\n{source}",
            sections.join("\n\n")
        )
    }
}

fn normalize_style_hint(style_hint: &str) -> Option<String> {
    let trimmed = style_hint.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.chars().take(1200).collect())
}

fn is_pending_translation(entry: &TranslationEntry) -> bool {
    !entry.source.trim().is_empty() && entry.target.trim().is_empty()
}

fn normalize_consistency_key(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_lowercase()
}

fn add_consistency_term(memory: &mut HashMap<String, ConsistencyTerm>, source: &str, target: &str) {
    let source = source.trim();
    let target = target.trim();
    if source.is_empty() || target.is_empty() {
        return;
    }
    let key = normalize_consistency_key(source);
    if key.len() < 3 {
        return;
    }
    memory.entry(key).or_insert_with(|| ConsistencyTerm {
        source: source.to_string(),
        target: target.to_string(),
    });
}

fn build_consistency_memory(entries: &[TranslationEntry]) -> HashMap<String, ConsistencyTerm> {
    let mut memory = HashMap::new();
    for entry in entries {
        if entry.source.trim().is_empty() || entry.target.trim().is_empty() {
            continue;
        }
        add_consistency_term(&mut memory, &entry.source, &entry.target);
        if let Some(variant) = split_series_variant(&entry.source) {
            if let Some(base_target) = strip_target_suffix(&entry.target, &variant.suffix) {
                add_consistency_term(&mut memory, &variant.base, &base_target);
            }
        }
    }
    memory
}

fn find_consistency_references(
    source: &str,
    memory: &HashMap<String, ConsistencyTerm>,
) -> Vec<ConsistencyTerm> {
    let normalized_source = normalize_consistency_key(source);
    let source_key = normalized_source.as_str();
    let mut refs: Vec<ConsistencyTerm> = memory
        .iter()
        .filter(|(key, _)| {
            key.len() >= 3 && source_key.contains(key.as_str()) && key.as_str() != source_key
        })
        .map(|(_, term)| term.clone())
        .collect();
    refs.sort_by(|a, b| b.source.len().cmp(&a.source.len()));
    refs.truncate(12);
    refs
}

fn split_series_variant(source: &str) -> Option<SeriesVariant> {
    let trimmed = source.trim();
    if let Some(split_idx) = trimmed
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| ch.is_whitespace().then_some(idx))
    {
        let base = trimmed[..split_idx].trim();
        let suffix = trimmed[split_idx..].trim();
        if is_series_base(base) && is_variant_suffix(suffix) {
            return Some(SeriesVariant {
                base: base.to_string(),
                suffix: suffix.to_string(),
            });
        }
    }

    let split_idx = find_compact_variant_suffix_start(trimmed)?;
    let base = trimmed[..split_idx].trim();
    let suffix = trimmed[split_idx..].trim();
    if !contains_cjk(base) {
        return None;
    }
    if !is_series_base(base) || !is_variant_suffix(suffix) {
        return None;
    }
    Some(SeriesVariant {
        base: base.to_string(),
        suffix: suffix.to_string(),
    })
}

fn is_series_base(base: &str) -> bool {
    if base.len() < 3 || !base.chars().any(|c| c.is_alphabetic()) {
        return false;
    }
    true
}

fn find_compact_variant_suffix_start(text: &str) -> Option<usize> {
    let mut start = text.len();
    let mut found = false;
    for (idx, ch) in text.char_indices().rev() {
        if ch.is_ascii_alphanumeric() {
            start = idx;
            found = true;
            continue;
        }
        break;
    }
    if !found || start == 0 || start == text.len() {
        return None;
    }
    is_variant_suffix(&text[start..]).then_some(start)
}

fn is_variant_suffix(suffix: &str) -> bool {
    let suffix = suffix
        .trim()
        .trim_start_matches('#')
        .trim_matches(|c| matches!(c, '(' | ')' | '[' | ']' | '{' | '}'));
    if suffix.is_empty() || suffix.len() > 8 {
        return false;
    }
    let has_digit = suffix.chars().any(|c| c.is_ascii_digit());
    has_digit
        && suffix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn strip_target_suffix(target: &str, suffix: &str) -> Option<String> {
    let target = target.trim();
    let suffix = suffix.trim();
    let without_suffix = target.strip_suffix(suffix)?.trim_end();
    let without_separator = without_suffix
        .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '_' | '#' | '：' | ':'));
    if without_separator.is_empty() {
        None
    } else {
        Some(without_separator.to_string())
    }
}

fn compose_variant_translation(base_target: &str, suffix: &str) -> String {
    let base_target = base_target.trim();
    let suffix = suffix.trim();
    if suffix.is_empty() {
        return base_target.to_string();
    }
    if suffix.starts_with(|c: char| c.is_ascii_punctuation()) {
        format!("{base_target}{suffix}")
    } else {
        format!("{base_target} {suffix}")
    }
}

fn contains_latin(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_alphabetic())
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c as u32,
            0x3400..=0x4dbf
                | 0x4e00..=0x9fff
                | 0xf900..=0xfaff
                | 0x20000..=0x2a6df
                | 0x2a700..=0x2b73f
                | 0x2b740..=0x2b81f
                | 0x2b820..=0x2ceaf
        )
    })
}

fn normalized_suffix(suffix: &str) -> String {
    suffix.trim().to_lowercase()
}

fn choose_latin_base(
    keys: &[String],
    profiles: &HashMap<String, SeriesBaseProfile>,
) -> Option<String> {
    keys.iter()
        .filter(|key| profiles.get(*key).is_some_and(|p| p.has_latin))
        .max_by_key(|key| {
            profiles
                .get(*key)
                .map(|p| (p.suffixes.len(), p.source.len()))
                .unwrap_or_default()
        })
        .cloned()
}

fn build_series_aliases(entries: &[TranslationEntry]) -> SeriesAliases {
    let mut profiles: HashMap<String, SeriesBaseProfile> = HashMap::new();
    let mut by_contentuid: HashMap<String, Vec<String>> = HashMap::new();

    for entry in entries {
        let Some(variant) = split_series_variant(&entry.source) else {
            continue;
        };
        let base_key = normalize_consistency_key(&variant.base);
        if base_key.is_empty() {
            continue;
        }
        let profile = profiles
            .entry(base_key.clone())
            .or_insert_with(|| SeriesBaseProfile {
                source: variant.base.clone(),
                suffixes: HashSet::new(),
                has_latin: false,
                has_cjk: false,
            });
        profile.suffixes.insert(normalized_suffix(&variant.suffix));
        profile.has_latin |= contains_latin(&variant.base);
        profile.has_cjk |= contains_cjk(&variant.base);

        if !entry.contentuid.trim().is_empty() {
            by_contentuid
                .entry(entry.contentuid.clone())
                .or_default()
                .push(base_key);
        }
    }

    let mut aliases = HashMap::new();
    for keys in by_contentuid.values_mut() {
        keys.sort();
        keys.dedup();
        let Some(canonical) = choose_latin_base(keys, &profiles) else {
            continue;
        };
        for key in keys {
            if key != &canonical && profiles.get(key).is_some_and(|p| p.has_cjk) {
                aliases.insert(key.clone(), canonical.clone());
            }
        }
    }

    let latin_keys: Vec<String> = profiles
        .iter()
        .filter(|(_, profile)| profile.has_latin)
        .map(|(key, _)| key.clone())
        .collect();
    let cjk_keys: Vec<String> = profiles
        .iter()
        .filter(|(_, profile)| profile.has_cjk && !profile.has_latin)
        .map(|(key, _)| key.clone())
        .collect();

    for cjk_key in cjk_keys {
        if aliases.contains_key(&cjk_key) {
            continue;
        }
        let Some(cjk_profile) = profiles.get(&cjk_key) else {
            continue;
        };
        let best = latin_keys
            .iter()
            .filter_map(|latin_key| {
                let latin_profile = profiles.get(latin_key)?;
                let overlap = cjk_profile
                    .suffixes
                    .intersection(&latin_profile.suffixes)
                    .count();
                (overlap >= 2).then_some((latin_key, overlap, latin_profile.suffixes.len()))
            })
            .max_by_key(|(_, overlap, suffix_count)| (*overlap, *suffix_count))
            .map(|(key, _, _)| key.clone());
        if let Some(canonical) = best {
            aliases.insert(cjk_key, canonical);
        }
    }

    let mut canonical_sources = HashMap::new();
    for (key, profile) in &profiles {
        canonical_sources
            .entry(key.clone())
            .or_insert_with(|| profile.source.clone());
    }
    for canonical in aliases.values() {
        if let Some(profile) = profiles.get(canonical) {
            canonical_sources.insert(canonical.clone(), profile.source.clone());
        }
    }

    SeriesAliases {
        aliases,
        canonical_sources,
    }
}

fn send_progress(on_event: &Channel<TranslationEvent>, entry_id: &str) {
    let _ = on_event.send(TranslationEvent::Progress {
        entry_id: entry_id.to_string(),
        status: TranslationStatus::Translating,
    });
}

fn send_done(on_event: &Channel<TranslationEvent>, entry_id: &str, text: String) {
    send_progress(on_event, entry_id);
    send_done_only(on_event, entry_id, text);
}

fn send_done_only(on_event: &Channel<TranslationEvent>, entry_id: &str, text: String) {
    let _ = on_event.send(TranslationEvent::Done {
        entry_id: entry_id.to_string(),
        text,
    });
}

fn send_error(on_event: &Channel<TranslationEvent>, entry_id: &str, message: String) {
    let _ = on_event.send(TranslationEvent::Error {
        entry_id: entry_id.to_string(),
        message,
    });
}

fn prepare_translation_jobs(
    entries: &[TranslationEntry],
    glossary: &crate::glossary::Glossary,
    on_event: &Channel<TranslationEvent>,
) -> (Vec<TranslationJob>, usize) {
    let pending: Vec<&TranslationEntry> = entries
        .iter()
        .filter(|e| is_pending_translation(e))
        .collect();
    let total = pending.len();
    let series_aliases = build_series_aliases(entries);
    let memory = build_consistency_memory(entries);
    let mut used_ids = HashSet::new();
    let mut jobs = Vec::new();

    let mut series_groups: HashMap<String, PendingSeriesGroup> = HashMap::new();
    let mut series_order = Vec::new();
    for entry in &pending {
        if let Some(variant) = split_series_variant(&entry.source) {
            let raw_base_key = normalize_consistency_key(&variant.base);
            let base_key = series_aliases.canonical_key(&raw_base_key);
            let base_source = series_aliases.source_for(&base_key, &variant.base);
            if let Some(term) = memory.get(&base_key) {
                send_done(
                    on_event,
                    &entry.id,
                    compose_variant_translation(&term.target, &variant.suffix),
                );
                used_ids.insert(entry.id.clone());
                continue;
            }
            if !series_groups.contains_key(&base_key) {
                series_order.push(base_key.clone());
            }
            series_groups
                .entry(base_key)
                .or_insert_with(|| PendingSeriesGroup {
                    base_source,
                    members: Vec::new(),
                })
                .members
                .push(SeriesMember {
                    entry_id: entry.id.clone(),
                    suffix: variant.suffix,
                });
        }
    }

    for key in series_order {
        let Some(group) = series_groups.remove(&key) else {
            continue;
        };
        if group.members.len() < 2 {
            continue;
        }
        for member in &group.members {
            used_ids.insert(member.entry_id.clone());
        }
        jobs.push(TranslationJob {
            matches: crate::glossary::find_matches(&group.base_source, glossary),
            consistency_terms: find_consistency_references(&group.base_source, &memory),
            source: group.base_source,
            output: TranslationOutput::Series {
                members: group.members,
            },
        });
    }

    let mut exact_groups: HashMap<String, Vec<&TranslationEntry>> = HashMap::new();
    let mut exact_order = Vec::new();
    for entry in &pending {
        if used_ids.contains(&entry.id) {
            continue;
        }
        let key = normalize_consistency_key(&entry.source);
        if let Some(term) = memory.get(&key) {
            send_done(on_event, &entry.id, term.target.clone());
            used_ids.insert(entry.id.clone());
            continue;
        }
        if !exact_groups.contains_key(&key) {
            exact_order.push(key.clone());
        }
        exact_groups.entry(key).or_default().push(entry);
    }

    for key in exact_order {
        let Some(group) = exact_groups.remove(&key) else {
            continue;
        };
        let Some(first) = group.first() else {
            continue;
        };
        let entry_ids: Vec<String> = group.iter().map(|e| e.id.clone()).collect();
        let output = if entry_ids.len() == 1 {
            TranslationOutput::Single {
                entry_id: entry_ids[0].clone(),
            }
        } else {
            TranslationOutput::ExactGroup { entry_ids }
        };
        jobs.push(TranslationJob {
            source: first.source.clone(),
            matches: crate::glossary::find_matches(&first.source, glossary),
            consistency_terms: find_consistency_references(&first.source, &memory),
            output,
        });
    }

    (jobs, total)
}

/// 批量流式翻译入口：逐条并发流式翻译。
///
/// - 只翻译 source 非空且 target 为空的条目（已翻译/已编辑的跳过）
/// - 用 Semaphore 控制并发数（同时 N 条在翻译）
/// - 通过 channel 实时推送每条的 start / delta / done / error
pub async fn translate_entries(
    client: &Client,
    settings: &LlmSettings,
    glossary: &crate::glossary::Glossary,
    entries: &[TranslationEntry],
    style_hint: String,
    on_event: &Channel<TranslationEvent>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<()> {
    let (jobs, total) = prepare_translation_jobs(entries, glossary, on_event);

    if total == 0 {
        let _ = on_event.send(TranslationEvent::AllDone {
            total: 0,
            failed: 0,
        });
        return Ok(());
    }

    let concurrency = settings.concurrency.clamp(1, 100);
    let semaphore = Arc::new(Semaphore::new(concurrency));

    let url = format!(
        "{}/v1/chat/completions",
        settings.base_url.trim_end_matches('/')
    );
    let api_key = settings.api_key.clone();
    let model = settings.model.clone();
    let style_hint = normalize_style_hint(&style_hint);

    // 为每个一致性任务创建独立并发任务。一个任务可能对应单条、重复条目组或同系列编号组。
    let mut tasks = Vec::new();
    for job in jobs {
        if is_cancelled(&cancel_flag) {
            break;
        }
        let permit = tokio::select! {
            permit = semaphore.clone().acquire_owned() => {
                permit.map_err(|e| AppError::Llm(format!("并发信号量错误: {e}")))?
            }
            _ = wait_until_cancelled(cancel_flag.clone()) => break,
        };
        if is_cancelled(&cancel_flag) {
            break;
        }
        let client = client.clone();
        let url = url.clone();
        let api_key = api_key.clone();
        let model = model.clone();
        let style_hint = style_hint.clone();
        let channel = on_event.clone();
        let cancel_flag = cancel_flag.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = permit; // 持有 permit 直到本任务完成
            run_translation_job(
                &client,
                &url,
                &api_key,
                &model,
                style_hint.as_deref(),
                job,
                &channel,
                cancel_flag,
            )
            .await
        }));
    }

    // 等待全部完成，统计失败条目数
    let mut failed_total = 0usize;
    for task in tasks {
        match task.await {
            Ok(Ok(failed)) => {
                failed_total += failed;
            }
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

async fn run_translation_job(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    style_hint: Option<&str>,
    job: TranslationJob,
    on_event: &Channel<TranslationEvent>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<usize> {
    let target_ids = job.target_ids();
    for entry_id in &target_ids {
        send_progress(on_event, entry_id);
    }

    let stream_entry_ids = match &job.output {
        TranslationOutput::Series { .. } => Vec::new(),
        _ => target_ids.clone(),
    };

    let translated = match translate_text_with_retry(
        client,
        url,
        api_key,
        model,
        &job.source,
        &job.matches,
        &job.consistency_terms,
        style_hint,
        &stream_entry_ids,
        on_event,
        cancel_flag,
    )
    .await
    {
        Ok(Some(text)) => text,
        Ok(None) => return Ok(0),
        Err(e) => {
            let message = e.to_string();
            for entry_id in &target_ids {
                send_error(on_event, entry_id, message.clone());
            }
            return Ok(target_ids.len());
        }
    };

    match job.output {
        TranslationOutput::Single { entry_id } => {
            send_done_only(on_event, &entry_id, translated);
        }
        TranslationOutput::ExactGroup { entry_ids } => {
            for entry_id in entry_ids {
                send_done_only(on_event, &entry_id, translated.clone());
            }
        }
        TranslationOutput::Series { members } => {
            for member in members {
                send_done_only(
                    on_event,
                    &member.entry_id,
                    compose_variant_translation(&translated, &member.suffix),
                );
            }
        }
    }

    Ok(0)
}

/// 翻译文本，带指数退避重试（最多 3 次）。
async fn translate_text_with_retry(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    source: &str,
    matches: &[crate::glossary::MatchedTerm],
    consistency_terms: &[ConsistencyTerm],
    style_hint: Option<&str>,
    stream_entry_ids: &[String],
    on_event: &Channel<TranslationEvent>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<Option<String>> {
    let mut last_err = String::new();
    for attempt in 1..=3u32 {
        if is_cancelled(&cancel_flag) {
            return Ok(None);
        }
        match translate_text_stream(
            client,
            url,
            api_key,
            model,
            source,
            matches,
            consistency_terms,
            style_hint,
            stream_entry_ids,
            on_event,
            cancel_flag.clone(),
        )
        .await
        {
            Ok(Some(text)) => return Ok(Some(text)),
            Ok(None) => return Ok(None),
            Err(e) => {
                last_err = e.to_string();
                log::warn!("[{source}] 第 {attempt} 次尝试失败: {last_err}");
                if attempt < 3 {
                    let backoff = Duration::from_millis(500 * 2u64.pow(attempt - 1));
                    tokio::select! {
                        _ = tokio::time::sleep(backoff) => {}
                        _ = wait_until_cancelled(cancel_flag.clone()) => return Ok(None),
                    }
                }
            }
        }
    }
    Err(AppError::Llm(last_err))
}

/// 流式翻译文本：每个 token 可实时推送到一个或多个前端条目。
///
/// 这是唯一文本翻译路径，确保：
/// 1. 每条都流式显示，前端即时反馈
/// 2. 重复条目可以共享同一次请求与同一译文
/// 3. Semaphore 控制实际并发吞吐
async fn translate_text_stream(
    client: &Client,
    url: &str,
    api_key: &str,
    model: &str,
    source: &str,
    matches: &[crate::glossary::MatchedTerm],
    consistency_terms: &[ConsistencyTerm],
    style_hint: Option<&str>,
    stream_entry_ids: &[String],
    on_event: &Channel<TranslationEvent>,
    cancel_flag: Arc<AtomicBool>,
) -> Result<Option<String>> {
    if is_cancelled(&cancel_flag) {
        return Ok(None);
    }

    let user_prompt = build_user_prompt(source, matches, consistency_terms, style_hint);
    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "temperature": 0.3,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": user_prompt }
        ]
    });

    let request = client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .timeout(Duration::from_secs(60))
        .send();
    let resp = tokio::select! {
        resp = request => resp.map_err(|e| AppError::Llm(format!("请求失败: {e}")))?,
        _ = wait_until_cancelled(cancel_flag.clone()) => return Ok(None),
    };

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

    loop {
        let chunk = tokio::select! {
            chunk = stream.next() => chunk,
            _ = wait_until_cancelled(cancel_flag.clone()) => return Ok(None),
        };
        let Some(chunk) = chunk else {
            break;
        };
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
                if is_cancelled(&cancel_flag) {
                    return Ok(None);
                }
                if !delta.is_empty() {
                    // 实时推送每个 token 到前端对应条目。系列组不推 delta，只推最终一致译文。
                    for entry_id in stream_entry_ids {
                        let _ = on_event.send(TranslationEvent::Delta {
                            entry_id: entry_id.clone(),
                            text: delta.clone(),
                        });
                    }
                    full.push_str(&delta);
                }
            }
        }
    }

    if is_cancelled(&cancel_flag) {
        return Ok(None);
    }

    Ok(Some(full.trim().to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_entry(source: &str, contentuid: &str) -> TranslationEntry {
        TranslationEntry {
            id: format!("{source}#{contentuid}"),
            source_file: "test.loca".into(),
            source: source.into(),
            target: String::new(),
            contentuid: contentuid.into(),
            version: String::new(),
            status: "pending".into(),
            error: None,
        }
    }

    #[test]
    fn user_prompt_includes_mod_style_hint() {
        let prompt = build_user_prompt(
            "Pose Pack",
            &[],
            &[],
            Some("博德之门实验室 MOD，Pose 按姿势名称翻译"),
        );
        assert!(prompt.contains("【本 MOD 翻译语境】"));
        assert!(prompt.contains("Pose 按姿势名称翻译"));
    }

    #[test]
    fn splits_numbered_series_variant() {
        let variant = split_series_variant("Silver's Hair 9b").unwrap();
        assert_eq!(variant.base, "Silver's Hair");
        assert_eq!(variant.suffix, "9b");
    }

    #[test]
    fn splits_compact_cjk_series_variant() {
        let variant = split_series_variant("银色发型9b").unwrap();
        assert_eq!(variant.base, "银色发型");
        assert_eq!(variant.suffix, "9b");
    }

    #[test]
    fn rejects_non_variant_tail_words() {
        assert!(split_series_variant("Silver's Hair Blonde").is_none());
    }

    #[test]
    fn strips_existing_target_suffix_for_memory() {
        assert_eq!(
            strip_target_suffix("银发 9b", "9b").as_deref(),
            Some("银发")
        );
        assert_eq!(strip_target_suffix("银发9", "9").as_deref(), Some("银发"));
    }

    #[test]
    fn composes_variant_translation_with_stable_spacing() {
        assert_eq!(compose_variant_translation("银发", "6"), "银发 6");
        assert_eq!(compose_variant_translation("银发", "9b"), "银发 9b");
    }

    #[test]
    fn aliases_cjk_series_to_latin_by_contentuid() {
        let entries = vec![
            test_entry("Silver's Hair 9b", "same-id"),
            test_entry("银色发型9b", "same-id"),
        ];
        let aliases = build_series_aliases(&entries);
        assert_eq!(
            aliases.canonical_key(&normalize_consistency_key("银色发型")),
            normalize_consistency_key("Silver's Hair")
        );
    }

    #[test]
    fn aliases_cjk_series_to_latin_by_shared_suffixes() {
        let entries = vec![
            test_entry("Silver's Hair 9b", "en-9b"),
            test_entry("Silver's Hair 10", "en-10"),
            test_entry("银色发型9b", "zh-9b"),
            test_entry("银色发型10", "zh-10"),
        ];
        let aliases = build_series_aliases(&entries);
        assert_eq!(
            aliases.canonical_key(&normalize_consistency_key("银色发型")),
            normalize_consistency_key("Silver's Hair")
        );
    }
}

// 保留旧的 TranslationStatus 引用，避免 unused import（types 模块导出用）
#[allow(unused_imports)]
use TranslationStatus as _UsedStatus;
