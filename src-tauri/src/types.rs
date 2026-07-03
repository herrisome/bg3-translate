use serde::{Deserialize, Serialize};

/// PAK 内文件类型分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PakFileKind {
    /// <contentList> 本地化 XML
    LocalizationXml,
    /// .loca 二进制本地化
    LocalizationLoca,
    /// .lsx 元数据
    MetadataLsx,
    /// Lua 脚本
    ScriptLua,
    /// stats 数据
    DataTxt,
    /// 其他
    Other,
}

/// PAK 内单个文件元信息（传给前端）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PakFile {
    pub name: String,
    pub size: u64,
    pub kind: PakFileKind,
    pub language: Option<String>,
}

/// 一个可翻译条目（XML/LSX/LOCA 的统一中间表示）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationEntry {
    pub id: String,
    pub source_file: String,
    pub source: String,
    pub target: String,
    pub contentuid: String,
    pub version: String,
    pub status: String,
    pub error: Option<String>,
}

/// 翻译状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranslationStatus {
    Pending,
    Translating,
    Translated,
    Edited,
    Error,
}

/// 流式翻译事件（通过 Tauri Channel 推送）
///
/// 注意：`#[serde(rename_all = "camelCase")]` 放在外层只作用于 variant 名，
/// variant 内部字段需要单独标注。这里统一用显式 rename 保证与前端 TS 类型一致。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TranslationEvent {
    /// 某条目状态变更
    Progress {
        #[serde(rename = "entryId")]
        entry_id: String,
        status: TranslationStatus,
    },
    /// 翻译增量文本
    Delta {
        #[serde(rename = "entryId")]
        entry_id: String,
        text: String,
    },
    /// 单条完成
    Done {
        #[serde(rename = "entryId")]
        entry_id: String,
        text: String,
    },
    /// 单条出错
    Error {
        #[serde(rename = "entryId")]
        entry_id: String,
        message: String,
    },
    /// 全部完成
    AllDone { total: usize, failed: usize },
}

/// LLM 设置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmSettings {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_base_url() -> String {
    "https://api.deepseek.com".into()
}
fn default_model() -> String {
    "deepseek-chat".into()
}
fn default_concurrency() -> usize {
    4
}
fn default_batch_size() -> usize {
    10
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
            api_key: String::new(),
            model: default_model(),
            concurrency: default_concurrency(),
            batch_size: default_batch_size(),
        }
    }
}

/// 解包结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractResult {
    pub work_dir: String,
    pub files: Vec<PakFile>,
}
