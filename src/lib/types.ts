/**
 * 前后端共享的类型定义。
 * 与 src-tauri/src/ 中的 Rust 结构体一一对应（serde 序列化）。
 */

/** PAK 内单个文件元信息 */
export interface PakFile {
  /** PAK 内路径，如 "Localization/English/foo.xml" */
  name: string;
  /** 解压后字节大小 */
  size: number;
  /** 文件类型分类（由后端识别） */
  kind: PakFileKind;
  /** 所属 BG3 语言（仅本地化文件有），如 "English" / "Polish" */
  language: string | null;
}

/** 文件类型分类，决定是否可翻译、如何翻译 */
export type PakFileKind =
  | "localization-xml" // <contentList> 本地化 XML
  | "localization-loca" // .loca 二进制本地化
  | "metadata-lsx" // .lsx 元数据
  | "script-lua" // Lua 脚本（一般不翻译）
  | "data-txt" // stats 数据
  | "other";

/** 一个可翻译条目（来自 XML/LSX/LOCA 的统一中间表示） */
export interface TranslationEntry {
  /** 条目唯一 ID：`{文件路径}#{contentuid}` */
  id: string;
  /** 所属 PAK 内文件路径 */
  sourceFile: string;
  /** 原文（英文） */
  source: string;
  /** 译文（中文），未翻译时为空 */
  target: string;
  /** contentuid（loca/xml 的 key），重打包时必须保留 */
  contentuid: string;
  /** 版本号 */
  version: string;
  /** 条目状态 */
  status: TranslationStatus;
  /** 错误信息（status=error 时） */
  error: string | null;
}

export type TranslationStatus =
  | "pending" // 待翻译
  | "translating" // 翻译中
  | "translated" // 已翻译
  | "edited" // 人工编辑过
  | "error"; // 出错

/** LLM 设置（OpenAI 兼容协议） */
export interface LlmSettings {
  baseUrl: string;
  apiKey: string;
  model: string;
  /** 并发数 */
  concurrency: number;
  /** 每批条目数 */
  batchSize: number;
}

/**
 * 流式翻译事件。Rust 端通过 Tauri Channel 推送，
 * 与 src-tauri/src/translation/events.rs 的 #[serde(tag="type")] 对齐。
 */
export type TranslationEvent =
  | { type: "progress"; entryId: string; status: TranslationStatus }
  | { type: "delta"; entryId: string; text: string }
  | { type: "done"; entryId: string; text: string }
  | { type: "error"; entryId: string; message: string }
  | { type: "all_done"; total: number; failed: number };

/** 解包结果 */
export interface ExtractResult {
  /** 解包临时目录（后端管理） */
  workDir: string;
  /** 文件列表 */
  files: PakFile[];
}

/** 调用后端的统一错误格式 */
export interface BackendError {
  message: string;
}

// ── 术语表 ──

/** 单条术语（与 Rust glossary::GlossaryEntry 字段一致） */
export interface GlossaryEntry {
  source: string;
  target: string;
  sourceKind: string;
  enabled: boolean;
  ambiguous: boolean;
  wholeWord: boolean;
  caseSensitive: boolean;
  count: number;
}

/** 整个术语表 */
export interface Glossary {
  terms: GlossaryEntry[];
}
