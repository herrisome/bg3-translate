import { create } from "zustand";
import type {
  LlmSettings,
  PakFile,
  TranslationEntry,
  TranslationStatus,
} from "@/lib/types";

/** 应用整体流程阶段 */
export type AppStage = "home" | "files" | "translate" | "done";

/** 按文件名缓存条目，key = PakFile.name */
type EntriesByFile = Record<string, TranslationEntry[]>;

interface AppState {
  // ── 流程状态 ──
  stage: AppStage;
  /** 原始 MOD 文件路径 */
  modFilePath: string | null;
  /** 工作目录（后端返回） */
  workDir: string | null;

  // ── 文件列表 ──
  files: PakFile[];
  /** 多选的文件列表 */
  selectedFiles: PakFile[];
  /** 已加载条目的文件名集合（避免重复加载） */
  loadedFileNames: Set<string>;

  // ── 翻译条目（按文件缓存）──
  entriesByFile: EntriesByFile;
  /** entryId → fileName 反查索引，加速 delta 高频更新 */
  entryIdToFile: Record<string, string>;
  /** 当前正在翻译的条目 id 集合 */
  translatingIds: Set<string>;

  // ── LLM 设置 ──
  settings: LlmSettings;
  settingsLoaded: boolean;

  // ── 加载态 ──
  loading: boolean;
  error: string | null;

  // ── Actions ──
  setStage: (stage: AppStage) => void;
  setModOpened: (filePath: string, workDir: string, files: PakFile[]) => void;
  /** 多选变化 */
  setSelectedFiles: (files: PakFile[]) => void;
  /** 设置某个文件的条目（加载后写入） */
  setFileEntries: (fileName: string, entries: TranslationEntry[]) => void;
  /** 更新单条（按 entry.id 定位） */
  updateEntry: (id: string, patch: Partial<TranslationEntry>) => void;
  setEntryStatus: (id: string, status: TranslationStatus) => void;
  appendDelta: (id: string, delta: string) => void;
  /** 获取所有选中文件的扁平化条目（派生） */
  getAllEntries: () => TranslationEntry[];
  /** 获取某个文件的条目，用于写回 */
  getFileEntries: (fileName: string) => TranslationEntry[];
  setSettings: (settings: LlmSettings) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

const DEFAULT_SETTINGS: LlmSettings = {
  baseUrl: "https://api.deepseek.com",
  apiKey: "",
  model: "deepseek-chat",
  concurrency: 6,
  batchSize: 10,
};

export const useAppStore = create<AppState>((set, get) => ({
  stage: "home",
  modFilePath: null,
  workDir: null,
  files: [],
  selectedFiles: [],
  loadedFileNames: new Set(),
  entriesByFile: {},
  entryIdToFile: {},
  translatingIds: new Set(),
  settings: DEFAULT_SETTINGS,
  settingsLoaded: false,
  loading: false,
  error: null,

  setStage: (stage) => set({ stage }),
  setModOpened: (filePath, workDir, files) =>
    set({
      modFilePath: filePath,
      workDir,
      files,
      stage: "files",
      selectedFiles: [],
      loadedFileNames: new Set(),
      entriesByFile: {},
      entryIdToFile: {},
      error: null,
    }),
  setSelectedFiles: (files) => set({ selectedFiles: files }),
  setFileEntries: (fileName, entries) =>
    set((s) => {
      // 建立 entryId → fileName 反查索引，避免 delta 高频更新时遍历所有条目
      const entryIdToFile = { ...s.entryIdToFile };
      for (const e of entries) entryIdToFile[e.id] = fileName;
      return {
        entriesByFile: { ...s.entriesByFile, [fileName]: entries },
        loadedFileNames: new Set([...s.loadedFileNames, fileName]),
        entryIdToFile,
      };
    }),
  updateEntry: (id, patch) =>
    set((s) => {
      const fileName = s.entryIdToFile[id];
      if (!fileName) return s;
      const list = s.entriesByFile[fileName];
      if (!list) return s;
      const idx = list.findIndex((e) => e.id === id);
      if (idx < 0) return s;
      const updated = [...list];
      updated[idx] = { ...updated[idx], ...patch };
      return {
        entriesByFile: { ...s.entriesByFile, [fileName]: updated },
      };
    }),
  setEntryStatus: (id, status) => get().updateEntry(id, { status }),
  appendDelta: (id, delta) =>
    set((s) => {
      const fileName = s.entryIdToFile[id];
      if (!fileName) return s;
      const list = s.entriesByFile[fileName];
      if (!list) return s;
      const idx = list.findIndex((e) => e.id === id);
      if (idx < 0) return s;
      const updated = [...list];
      updated[idx] = {
        ...updated[idx],
        target: updated[idx].target + delta,
        status: "translating",
      };
      return {
        entriesByFile: { ...s.entriesByFile, [fileName]: updated },
      };
    }),
  getAllEntries: () => {
    const { selectedFiles, entriesByFile } = get();
    const all: TranslationEntry[] = [];
    for (const f of selectedFiles) {
      const list = entriesByFile[f.name];
      if (list) all.push(...list);
    }
    return all;
  },
  getFileEntries: (fileName) => get().entriesByFile[fileName] ?? [],
  setSettings: (settings) => set({ settings, settingsLoaded: true }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  reset: () =>
    set({
      stage: "home",
      modFilePath: null,
      workDir: null,
      files: [],
      selectedFiles: [],
      loadedFileNames: new Set(),
      entriesByFile: {},
      entryIdToFile: {},
      translatingIds: new Set(),
      error: null,
    }),
}));
