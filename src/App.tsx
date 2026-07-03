import { useEffect, useState } from "react";
import {
  CheckCircle2,
  ChevronRight,
  Languages,
  Package,
  Shield,
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
import { FileTree, TRANSLATABLE } from "@/components/FileTree";
import { SettingsPanel } from "@/components/SettingsPanel";
import { TranslationTable } from "@/components/TranslationTable";
import { useAppStore } from "@/store/app-store";
import { pickSavePath, repackMod, writeFileEntries } from "@/lib/tauri";
import type { AppStage } from "@/store/app-store";

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

function Header() {
  return (
    <header className="flex items-center justify-between border-b px-6 py-3">
      <div className="flex items-center gap-2">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
          <Languages className="h-4 w-4" />
        </div>
        <div>
          <h1 className="text-sm font-bold leading-tight">BG3 MOD 汉化工具</h1>
          <p className="text-[10px] text-muted-foreground">
            博德之门3 MOD 本地化翻译
          </p>
        </div>
      </div>
      <Stepper />
    </header>
  );
}

function HomePage() {
  return (
    <div className="grid gap-4 p-6 md:grid-cols-2">
      <FileDropZone />
      <div className="space-y-4">
        <SettingsPanel />
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
    </div>
  );
}

function FilesPage() {
  const files = useAppStore((s) => s.files);
  const selectedFile = useAppStore((s) => s.selectedFile);
  const selectFile = useAppStore((s) => s.selectFile);
  const workDir = useAppStore((s) => s.workDir);
  const entries = useAppStore((s) => s.entries);
  const setStage = useAppStore((s) => s.setStage);
  const setError = useAppStore((s) => s.setError);

  const onGoPack = async () => {
    if (!workDir) return;
    // 先把当前编辑写回文件
    if (selectedFile && entries.length > 0) {
      try {
        await writeFileEntries(workDir, selectedFile.name, entries);
      } catch (e) {
        setError(String(e));
        return;
      }
    }
    setStage("done");
  };

  return (
    <div className="grid min-h-0 flex-1 grid-cols-[280px_1fr] gap-0">
      <div className="min-h-0 border-r">
        <FileTree
          files={files}
          selectedFile={selectedFile}
          onSelect={(f) => selectFile(f)}
        />
      </div>
      <div className="flex min-h-0 flex-1 flex-col">
        <div className="min-h-0 flex-1">
          <TranslationTable />
        </div>
        {/* 固定在底部的操作栏 */}
        <div className="flex shrink-0 items-center justify-between border-t bg-muted/30 px-4 py-3">
          <Button variant="ghost" onClick={() => setStage("home")}>
            <ChevronRight className="h-4 w-4 rotate-180" />
            返回
          </Button>
          <Button onClick={onGoPack} disabled={!workDir}>
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

  const translatable = files.filter((f) => TRANSLATABLE.includes(f.kind));

  return (
    <div className="flex items-center justify-center p-6">
      <Card className="max-w-lg">
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

  // 错误 toast（简化版：顶部 banner）
  useEffect(() => {
    if (error) {
      const t = setTimeout(() => setError(null), 5000);
      return () => clearTimeout(t);
    }
  }, [error, setError]);

  return (
    <div className="flex h-screen flex-col bg-background">
      <Header />
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
        {stage === "home" && <HomePage />}
        {stage === "files" && <FilesPage />}
        {stage === "done" && <DonePage />}
      </div>
    </div>
  );
}

export default App;
