import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import { Archive, FolderOpen, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  extractMod,
  openMod,
  pickExtractDirectory,
  pickModFile,
} from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useAppStore } from "@/store/app-store";

export function FileDropZone({ className }: { className?: string }) {
  const setModOpened = useAppStore((s) => s.setModOpened);
  const setLoading = useAppStore((s) => s.setLoading);
  const setError = useAppStore((s) => s.setError);
  const loading = useAppStore((s) => s.loading);
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);
  const [extracting, setExtracting] = useState(false);
  const [extractResult, setExtractResult] = useState<{
    dir: string;
    count: number;
  } | null>(null);

  const handleOpen = useCallback(
    async (filePath: string) => {
      setLoading(true);
      setError(null);
      setExtractResult(null);
      try {
        const result = await openMod(filePath);
        setModOpened(filePath, result.workDir, result.files);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    },
    [setModOpened, setLoading, setError],
  );

  const onClickPick = useCallback(async () => {
    const picked = await pickModFile();
    if (picked) await handleOpen(picked);
  }, [handleOpen]);

  const onClickExtract = useCallback(async () => {
    const picked = await pickModFile();
    if (!picked) return;
    const outputDir = await pickExtractDirectory();
    if (!outputDir) return;

    setExtracting(true);
    setError(null);
    setExtractResult(null);
    try {
      const files = await extractMod(picked, outputDir);
      setExtractResult({ dir: outputDir, count: files.length });
    } catch (e) {
      setError(String(e));
    } finally {
      setExtracting(false);
    }
  }, [setError]);

  const openDroppedPath = useCallback(
    async (paths: string[]) => {
      const filePath = paths.find((path) => /\.(pak|zip)$/i.test(path));
      if (!filePath) {
        setError("请拖入 .pak 或 .zip 文件");
        return;
      }
      await handleOpen(filePath);
    },
    [handleOpen, setError],
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload: DragDropEvent = event.payload;
        if (payload.type === "enter" || payload.type === "over") {
          setDragOver(true);
          return;
        }
        if (payload.type === "leave") {
          setDragOver(false);
          return;
        }
        setDragOver(false);
        void openDroppedPath(payload.paths);
      })
      .then((cleanup) => {
        if (disposed) cleanup();
        else unlisten = cleanup;
      })
      .catch(() => {
        /* 浏览器预览环境没有 Tauri webview 事件，保留 HTML5 兜底。 */
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [openDroppedPath]);

  // 浏览器预览兜底；Tauri 桌面端真实路径由 onDragDropEvent 提供。
  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const f = e.dataTransfer.files?.[0];
      if (f && /\.(pak|zip)$/i.test(f.name)) {
        const path = (f as unknown as { path?: string }).path;
        if (path) handleOpen(path);
      }
    },
    [handleOpen],
  );

  return (
    <Card
      className={cn(
        "flex min-h-[420px] cursor-pointer flex-col items-center justify-center border-2 border-dashed p-8 text-center transition-colors md:min-h-[520px]",
        dragOver
          ? "border-primary bg-accent/50"
          : "border-border hover:border-primary/50",
        className,
      )}
      onDragOver={(e) => {
        e.preventDefault();
        setDragOver(true);
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={onDrop}
      onClick={loading || extracting ? undefined : onClickPick}
    >
      <div className="flex flex-col items-center justify-center gap-5">
        <div
          className={`rounded-2xl bg-primary/10 p-6 transition-transform ${
            dragOver ? "scale-110" : ""
          }`}
        >
          {loading || extracting ? (
            <Loader2 className="h-12 w-12 animate-spin text-primary" />
          ) : (
            <FolderOpen className="h-12 w-12 text-primary" />
          )}
        </div>
        <div className="space-y-1">
          <h2 className="text-lg font-semibold md:text-xl">
            {loading
              ? "正在解包…"
              : extracting
                ? "正在仅解压…"
                : "打开 MOD 文件"}
          </h2>
          <p className="text-sm text-muted-foreground">
            {loading
              ? "请稍候，正在解析 PAK 内容"
              : extracting
                ? "请稍候，正在写入选择的输出目录"
                : "拖拽 .pak 或 .zip 文件到此区域，或点击选择"}
          </p>
        </div>
        {!loading && !extracting && (
          <div className="flex flex-wrap items-center justify-center gap-3">
            <Button
              size="lg"
              onClick={(e) => {
                e.stopPropagation();
                onClickPick();
              }}
            >
              <FolderOpen className="h-4 w-4" />
              选择文件
            </Button>
            <Button
              size="lg"
              variant="outline"
              onClick={(e) => {
                e.stopPropagation();
                onClickExtract();
              }}
            >
              <Archive className="h-4 w-4" />
              仅解压
            </Button>
          </div>
        )}
        {extractResult && (
          <div className="max-w-xl rounded-md border bg-muted/30 px-4 py-3 text-left text-xs leading-5">
            <div className="font-medium text-foreground">
              已解压 {extractResult.count} 个文件
            </div>
            <div className="mt-1 break-all text-muted-foreground">
              {extractResult.dir}
            </div>
          </div>
        )}
        <p className="max-w-sm text-xs text-muted-foreground">
          支持 Nexus 标准打包的 .zip 和 BG3 原生 .pak 格式
        </p>
      </div>
      <input
        ref={inputRef}
        type="file"
        accept=".pak,.zip"
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          const path = (f as unknown as { path?: string })?.path;
          if (path) handleOpen(path);
        }}
      />
    </Card>
  );
}
