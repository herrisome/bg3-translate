import { invoke, Channel } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type {
  ExtractResult,
  Glossary,
  GlossaryEntry,
  LlmSettings,
  PakFile,
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

/** 选择解压输出目录 */
export async function pickExtractDirectory(): Promise<string | null> {
  const selected = await openDialog({
    multiple: false,
    directory: true,
  });
  return typeof selected === "string" ? selected : null;
}

// ─────────────────────────────────────────────────────────────
// 后端命令封装
// ─────────────────────────────────────────────────────────────

/** 打开并解包 MOD 文件（.pak 或 .zip），返回文件列表 */
export async function openMod(filePath: string): Promise<ExtractResult> {
  return invoke<ExtractResult>("open_mod", { filePath });
}

/** 仅解压 MOD 文件到指定目录 */
export async function extractMod(
  filePath: string,
  outputDir: string,
): Promise<PakFile[]> {
  return invoke<PakFile[]>("extract_mod", { filePath, outputDir });
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
  styleHint: string,
  onEvent: (e: TranslationEvent) => void,
): Promise<void> {
  const channel = new Channel<TranslationEvent>();
  channel.onmessage = onEvent;
  await invoke("translate_entries", {
    workDir,
    entries,
    styleHint,
    onEvent: channel,
  });
}

/** 请求取消当前翻译任务 */
export async function cancelTranslation(): Promise<void> {
  await invoke("cancel_translation");
}

// ─────────────────────────────────────────────────────────────
// 术语表
// ─────────────────────────────────────────────────────────────

/** 读取术语表 */
export async function listGlossary(): Promise<Glossary> {
  return invoke<Glossary>("list_glossary");
}

/** 新增术语 */
export async function addGlossaryEntry(entry: GlossaryEntry): Promise<Glossary> {
  return invoke<Glossary>("add_glossary_entry", { entry });
}

/** 更新术语 */
export async function updateGlossaryEntry(
  oldSource: string,
  entry: GlossaryEntry,
): Promise<Glossary> {
  return invoke<Glossary>("update_glossary_entry", { oldSource, entry });
}

/** 删除术语 */
export async function deleteGlossaryEntry(source: string): Promise<Glossary> {
  return invoke<Glossary>("delete_glossary_entry", { source });
}

/** 重置术语表为官方种子 */
export async function resetGlossary(): Promise<Glossary> {
  return invoke<Glossary>("reset_glossary");
}

/** 导入术语表 JSON（从游戏提取的完整官方术语表） */
export async function importGlossary(jsonStr: string): Promise<Glossary> {
  return invoke<Glossary>("import_glossary", { jsonStr });
}
