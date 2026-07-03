use serde::Serialize;

/// 统一错误类型，可序列化为前端可读字符串。
/// Tauri 命令返回 Result<T, AppError>，通过 Serialize 转换为字符串。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("PAK 处理错误: {0}")]
    Pak(String),

    #[error("PAK 库错误: {0}")]
    PakLib(#[from] bg3rustpaklib::PakError),

    #[error("LOCA 处理错误: {0}")]
    Loca(#[from] bg3rustpaklib::loca::LocaError),

    #[error("XML 解析错误: {0}")]
    Xml(String),

    #[error("LLM 调用错误: {0}")]
    Llm(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("{0}")]
    Other(String),
}

// 让 AppError 能作为 Tauri 命令的错误返回（序列化为字符串）
impl Serialize for AppError {
    fn serialize<S>(&self, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
