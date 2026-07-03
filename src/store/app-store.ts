import { create } from "zustand";
import type {
  LlmSettings,
  PakFile,
  TranslationEntry,
  TranslationStatus,
} from "@/lib/types";

/** 应用整体流程阶段 */
export type AppStage = "home" | "files" | "translate" | "done";

interface AppState {
  // ── 流程状态 ──
  stage: AppStage;
  /** 原始 MOD 文件路径 */
  modFilePath: string | null;
  /** 工作目录（后端返回） */
  workDir: string | null;

  // ── 文件列表 ──
  files: PakFile[];
  /** 当前选中查看的文件 */
  selectedFile: PakFile | null;

  // ── 翻译条目 ──
  entries: TranslationEntry[];
  /** 翻译进度（已完成的条目 id 集合） */
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
  selectFile: (file: PakFile | null) => void;
  setEntries: (entries: TranslationEntry[]) => void;
  updateEntry: (id: string, patch: Partial<TranslationEntry>) => void;
  setEntryStatus: (id: string, status: TranslationStatus) => void;
  appendDelta: (id: string, delta: string) => void;
  setSettings: (settings: LlmSettings) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  reset: () => void;
}

const DEFAULT_SETTINGS: LlmSettings = {
  baseUrl: "https://api.deepseek.com",
  apiKey: "",
  model: "deepseek-chat",
  concurrency: 4,
  batchSize: 10,
};

export const useAppStore = create<AppState>((set) => ({
  stage: "home",
  modFilePath: null,
  workDir: null,
  files: [],
  selectedFile: null,
  entries: [],
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
      selectedFile: null,
      entries: [],
      error: null,
    }),
  selectFile: (file) =>
    set({ selectedFile: file, entries: file ? [] : [] }),
  setEntries: (entries) => set({ entries }),
  updateEntry: (id, patch) =>
    set((s) => ({
      entries: s.entries.map((e) =>
        e.id === id ? { ...e, ...patch } : e,
      ),
    })),
  setEntryStatus: (id, status) =>
    set((s) => ({
      entries: s.entries.map((e) =>
        e.id === id ? { ...e, status } : e,
      ),
    })),
  appendDelta: (id, delta) =>
    set((s) => ({
      entries: s.entries.map((e) =>
        e.id === id
          ? { ...e, target: e.target + delta, status: "translating" }
          : e,
      ),
    })),
  setSettings: (settings) => set({ settings, settingsLoaded: true }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  reset: () =>
    set({
      stage: "home",
      modFilePath: null,
      workDir: null,
      files: [],
      selectedFile: null,
      entries: [],
      translatingIds: new Set(),
      error: null,
    }),
}));
