import { invoke, Channel } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type {
  ExtractResult,
  LlmSettings,
  TranslationEntry,
  TranslationEvent,
} from "./types";

// ─────────────────────────────────────────────────────────────
// 文件对话框封装
// ─────────────────────────────────────────────────────────────

/** 选择一个 .pak 或 .zip 文件 */
export async function pickModFile(): Promise<string | null> {
  const selected = await openDialog({
    multiple: false,
    filters: [{ name: "BG3 MOD", extensions: ["pak", "zip"] }],
  });
  return typeof selected === "string" ? selected : null;
}

/** 选择保存位置 */
export async function pickSavePath(defaultName: string): Promise<string | null> {
  return await saveDialog({
    defaultPath: defaultName,
    filters: [{ name: "BG3 PAK", extensions: ["pak"] }],
  });
}

// ─────────────────────────────────────────────────────────────
// 后端命令封装
// ─────────────────────────────────────────────────────────────

/** 打开并解包 MOD 文件（.pak 或 .zip），返回文件列表 */
export async function openMod(filePath: string): Promise<ExtractResult> {
  return invoke<ExtractResult>("open_mod", { filePath });
}

/** 读取指定 PAK 文件的可翻译条目（XML/LSX/LOCA 统一为 TranslationEntry） */
export async function readFileEntries(
  workDir: string,
  fileName: string,
): Promise<TranslationEntry[]> {
  return invoke<TranslationEntry[]>("read_file_entries", { workDir, fileName });
}

/** 保存编辑后的条目回工作目录文件 */
export async function writeFileEntries(
  workDir: string,
  fileName: string,
  entries: TranslationEntry[],
): Promise<void> {
  await invoke("write_file_entries", { workDir, fileName, entries });
}

/** 重新打包工作目录为 .pak */
export async function repackMod(
  workDir: string,
  outputPath: string,
): Promise<void> {
  await invoke("repack_mod", { workDir, outputPath });
}

// ─────────────────────────────────────────────────────────────
// LLM 翻译
// ─────────────────────────────────────────────────────────────

/** 保存 LLM 设置到后端（持久化） */
export async function saveLlmSettings(settings: LlmSettings): Promise<void> {
  await invoke("save_llm_settings", { settings });
}

/** 读取已保存的 LLM 设置 */
export async function loadLlmSettings(): Promise<LlmSettings> {
  return invoke<LlmSettings>("load_llm_settings");
}

/**
 * 流式翻译条目。通过 Tauri Channel 接收实时事件。
 *
 * @param workDir 工作目录
 * @param entries 待翻译条目（只翻译 source 非空且 target 为空的）
 * @param onEvent 事件回调
 */
export async function translateEntries(
  workDir: string,
  entries: TranslationEntry[],
  onEvent: (e: TranslationEvent) => void,
): Promise<void> {
  const channel = new Channel<TranslationEvent>();
  channel.onmessage = onEvent;
  await invoke("translate_entries", { workDir, entries, onEvent: channel });
}
