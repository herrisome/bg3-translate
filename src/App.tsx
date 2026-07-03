import { useEffect, useState } from "react";
import {
  CheckCircle2,
  ChevronRight,
  Languages,
  Package,
  Shield,
  BookOpen,
  Settings,
  Palette,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { FileDropZone } from "@/components/FileDropZone";
import { FileTree, LOCALIZATION_KINDS } from "@/components/FileTree";
import { GlossaryPanel } from "@/components/GlossaryPanel";
import { SettingsPanel } from "@/components/SettingsPanel";
import { TranslationTable } from "@/components/TranslationTable";
import { useAppStore } from "@/store/app-store";
import { pickSavePath, repackMod, writeFileEntries } from "@/lib/tauri";
import type { AppStage, Theme } from "@/store/app-store";
import { THEMES } from "@/store/app-store";

const STEPS: { stage: AppStage; label: string; icon: React.ReactNode }[] = [
  { stage: "home", label: "选择 MOD", icon: <Package className="h-4 w-4" /> },
  { stage: "files", label: "翻译", icon: <Languages className="h-4 w-4" /> },
  { stage: "done", label: "打包", icon: <CheckCircle2 className="h-4 w-4" /> },
];

function Stepper() {
  const stage = useAppStore((s) => s.stage);
  const currentIndex = STEPS.findIndex((s) => s.stage === stage);
  return (
    <div className="flex items-center gap-1 text-sm">
      {STEPS.map((step, i) => (
        <div key={step.stage} className="flex items-center">
          <div
            className={`flex items-center gap-1.5 rounded-full px-3 py-1 ${
              i === currentIndex
                ? "bg-primary text-primary-foreground"
                : i < currentIndex
                  ? "text-muted-foreground"
                  : "text-muted-foreground/50"
            }`}
          >
            {step.icon}
            <span>{step.label}</span>
          </div>
          {i < STEPS.length - 1 && (
            <ChevronRight className="mx-1 h-3 w-3 text-muted-foreground/40" />
          )}
        </div>
      ))}
    </div>
  );
}

function ThemeSwitcher() {
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  return (
    <div className="relative">
      <select
        value={theme}
        onChange={(e) => setTheme(e.target.value as Theme)}
        className="h-8 cursor-pointer rounded-md border border-input bg-background pl-7 pr-6 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
        title="切换主题"
      >
        {THEMES.map((t) => (
          <option key={t.value} value={t.value}>
            {t.label}
          </option>
        ))}
      </select>
      <Palette className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
    </div>
  );
}

function Header({
  onOpenGlossary,
  onOpenSettings,
}: {
  onOpenGlossary: () => void;
  onOpenSettings: () => void;
}) {
  const settings = useAppStore((s) => s.settings);
  const configured = settings.apiKey.length > 0;
  return (
    <header className="flex items-center justify-between border-b px-4 py-2.5 md:px-6">
      <div className="flex items-center gap-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
          <Languages className="h-4 w-4" />
        </div>
        <div className="hidden sm:block">
          <h1 className="text-sm font-bold leading-tight">BG3 MOD 汉化工具</h1>
          <p className="text-[10px] text-muted-foreground">
            博德之门3 MOD 本地化翻译
          </p>
        </div>
      </div>
      <div className="flex items-center gap-2 md:gap-3">
        <div className="hidden lg:block">
          <Stepper />
        </div>
        <ThemeSwitcher />
        <Button size="sm" variant="outline" onClick={onOpenSettings}>
          <Settings className="h-4 w-4" />
          <span className="hidden md:inline">
            {configured ? settings.model : "未配置"}
          </span>
          <span
            className={`ml-1 inline-block h-2 w-2 rounded-full ${
              configured ? "bg-emerald-500" : "bg-amber-500"
            }`}
          />
        </Button>
        <Button size="sm" variant="outline" onClick={onOpenGlossary}>
          <BookOpen className="h-4 w-4" />
          <span className="hidden md:inline">术语表</span>
        </Button>
      </div>
    </header>
  );
}

function HomePage({ onOpenSettings }: { onOpenSettings: () => void }) {
  const settings = useAppStore((s) => s.settings);
  const configured = settings.apiKey.length > 0;
  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-4 p-4 md:p-8">
      <FileDropZone />
      {/* 紧凑的配置状态条 */}
      <Card>
        <CardContent className="flex items-center justify-between p-4">
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <Settings className="h-4 w-4 shrink-0 text-muted-foreground" />
              <span className="truncate text-sm font-medium">
                {configured ? settings.model : "未配置大模型"}
              </span>
              <span
                className={`shrink-0 rounded-full px-2 py-0.5 text-[10px] ${
                  configured
                    ? "bg-emerald-500/15 text-emerald-600"
                    : "bg-amber-500/15 text-amber-600"
                }`}
              >
                {configured ? "已就绪" : "需配置"}
              </span>
            </div>
            <p className="mt-0.5 truncate text-xs text-muted-foreground">
              {configured ? settings.baseUrl : "点击右侧按钮配置 API（支持 DeepSeek/智谱/OpenAI 等）"}
            </p>
          </div>
          <Button size="sm" variant="outline" className="ml-3 shrink-0" onClick={onOpenSettings}>
            配置
          </Button>
        </CardContent>
      </Card>
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-base">
            <Shield className="h-4 w-4" />
            工作流程
          </CardTitle>
        </CardHeader>
        <CardContent>
          <ol className="space-y-2 text-sm text-muted-foreground">
            <li>1. 选择 MOD 文件（.pak 或 .zip）</li>
            <li>2. 配置大模型 API（OpenAI 兼容协议）</li>
            <li>3. 自动解包，识别可翻译的本地化文件</li>
            <li>4. 流式翻译，人工校对</li>
            <li>5. 原样重新打包为 .pak</li>
          </ol>
        </CardContent>
      </Card>
    </div>
  );
}

function FilesPage() {
  const files = useAppStore((s) => s.files);
  const selectedFiles = useAppStore((s) => s.selectedFiles);
  const setSelectedFiles = useAppStore((s) => s.setSelectedFiles);
  const workDir = useAppStore((s) => s.workDir);
  const entriesByFile = useAppStore((s) => s.entriesByFile);
  const setStage = useAppStore((s) => s.setStage);
  const setError = useAppStore((s) => s.setError);

  const onGoPack = async () => {
    if (!workDir) return;
    // 把所有已加载、有内容的文件条目写回工作目录
    try {
      for (const f of selectedFiles) {
        const entries = entriesByFile[f.name];
        if (entries && entries.length > 0) {
          await writeFileEntries(workDir, f.name, entries);
        }
      }
    } catch (e) {
      setError(String(e));
      return;
    }
    setStage("done");
  };

  return (
    <div className="grid min-h-0 flex-1 grid-cols-1 gap-0 lg:grid-cols-[280px_1fr]">
      {/* 窄屏时文件树折叠为顶部条；宽屏时为左侧栏 */}
      <div className="min-h-0 border-b lg:border-b-0 lg:border-r">
        <FileTree
          files={files}
          selectedFiles={selectedFiles}
          onSelectionChange={setSelectedFiles}
        />
      </div>
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <div className="min-h-0 flex-1">
          <TranslationTable />
        </div>
        {/* 固定在底部的操作栏 */}
        <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t bg-muted/30 px-4 py-3">
          <Button variant="ghost" onClick={() => setStage("home")}>
            <ChevronRight className="h-4 w-4 rotate-180" />
            <span className="hidden sm:inline">返回</span>
          </Button>
          <Button onClick={onGoPack} disabled={!workDir || selectedFiles.length === 0}>
            <CheckCircle2 className="h-4 w-4" />
            完成翻译，去打包
          </Button>
        </div>
      </div>
    </div>
  );
}

function DonePage() {
  const modFilePath = useAppStore((s) => s.modFilePath);
  const workDir = useAppStore((s) => s.workDir);
  const files = useAppStore((s) => s.files);
  const setStage = useAppStore((s) => s.setStage);
  const setError = useAppStore((s) => s.setError);
  const reset = useAppStore((s) => s.reset);
  const [packing, setPacking] = useState(false);
  const [outputPath, setOutputPath] = useState<string | null>(null);

  const onPack = async () => {
    if (!workDir || !modFilePath) return;
    setPacking(true);
    setError(null);
    try {
      const baseName = modFilePath
        .split("/")
        .pop()!
        .replace(/\.(pak|zip)$/i, "");
      const savePath = await pickSavePath(`${baseName}_zh.pak`);
      if (!savePath) {
        setPacking(false);
        return;
      }
      await repackMod(workDir, savePath);
      setOutputPath(savePath);
    } catch (e) {
      setError(String(e));
    } finally {
      setPacking(false);
    }
  };

  const translatable = files.filter((f) => LOCALIZATION_KINDS.includes(f.kind));

  return (
    <div className="mx-auto flex max-w-2xl items-center justify-center p-4 md:p-8">
      <Card className="w-full">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Package className="h-5 w-5" />
            重新打包
          </CardTitle>
          <CardDescription>
            将翻译后的文件重新打包为 BG3 可加载的 .pak 格式
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="rounded-lg bg-muted/50 p-3 text-sm">
            <div className="mb-1 text-muted-foreground">源 MOD：</div>
            <div className="break-all">{modFilePath}</div>
            <div className="mt-3 mb-1 text-muted-foreground">
              可翻译文件数：{translatable.length}
            </div>
          </div>

          {outputPath ? (
            <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-3 text-sm">
              <div className="flex items-center gap-2 font-medium text-emerald-600 dark:text-emerald-400">
                <CheckCircle2 className="h-4 w-4" />
                打包完成！
              </div>
              <div className="mt-1 break-all text-muted-foreground">
                {outputPath}
              </div>
              <p className="mt-2 text-xs text-muted-foreground">
                将此 .pak 文件放入 BG3 的 Mods 文件夹，或在 mod 管理器中导入。
              </p>
            </div>
          ) : (
            <Button onClick={onPack} disabled={packing} className="w-full" size="lg">
              {packing ? (
                <>
                  <Package className="h-4 w-4 animate-pulse" />
                  打包中…
                </>
              ) : (
                <>
                  <Package className="h-4 w-4" />
                  选择保存位置并打包
                </>
              )}
            </Button>
          )}

          <div className="flex gap-2">
            <Button
              variant="outline"
              className="flex-1"
              onClick={() => setStage("files")}
            >
              返回继续编辑
            </Button>
            <Button variant="ghost" className="flex-1" onClick={reset}>
              开始新的 MOD
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

function App() {
  const stage = useAppStore((s) => s.stage);
  const error = useAppStore((s) => s.error);
  const setError = useAppStore((s) => s.setError);
  const [showGlossary, setShowGlossary] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

  // 错误 toast（简化版：顶部 banner）
  useEffect(() => {
    if (error) {
      const t = setTimeout(() => setError(null), 5000);
      return () => clearTimeout(t);
    }
  }, [error, setError]);

  return (
    <div className="relative flex h-screen flex-col bg-background">
      <Header
        onOpenGlossary={() => setShowGlossary(true)}
        onOpenSettings={() => setShowSettings(true)}
      />
      {error && (
        <div className="bg-destructive px-6 py-2 text-sm text-destructive-foreground">
          ⚠ {error}
        </div>
      )}
      {/* 翻译页需要内部固定布局（按钮固定底部），不使用外层滚动；
          首页/完成页内容可滚动 */}
      <div
        className={
          stage === "files"
            ? "flex min-h-0 flex-1 flex-col"
            : "flex-1 overflow-auto"
        }
      >
        {stage === "home" && <HomePage onOpenSettings={() => setShowSettings(true)} />}
        {stage === "files" && <FilesPage />}
        {stage === "done" && <DonePage />}
      </div>
      {/* 设置面板（覆盖层）*/}
      {showSettings && (
        <div className="absolute inset-0 z-40 flex items-start justify-center overflow-auto bg-black/40 p-4 md:p-8">
          <div className="w-full max-w-md">
            <SettingsPanel compact={false} />
            <div className="mt-3 flex justify-center">
              <Button onClick={() => setShowSettings(false)}>完成</Button>
            </div>
          </div>
        </div>
      )}
      {/* 术语表面板（覆盖层）*/}
      {showGlossary && (
        <div className="absolute inset-0 z-40 bg-background">
          <GlossaryPanel onClose={() => setShowGlossary(false)} />
        </div>
      )}
    </div>
  );
}

export default App;
