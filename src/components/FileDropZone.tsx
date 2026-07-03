import { useCallback, useRef, useState } from "react";
import { FolderOpen, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { pickModFile, openMod } from "@/lib/tauri";
import { useAppStore } from "@/store/app-store";

export function FileDropZone() {
  const setModOpened = useAppStore((s) => s.setModOpened);
  const setLoading = useAppStore((s) => s.setLoading);
  const setError = useAppStore((s) => s.setError);
  const loading = useAppStore((s) => s.loading);
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragOver, setDragOver] = useState(false);

  const handleOpen = useCallback(
    async (filePath: string) => {
      setLoading(true);
      setError(null);
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

  // 拖拽：Tauri v2 桌面端，浏览器的 dragdrop 事件可用
  const onDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setDragOver(false);
      const f = e.dataTransfer.files?.[0];
      if (f && /\.(pak|zip)$/i.test(f.name)) {
        // Tauri v2 中通过 path 属性可拿到绝对路径
        const path = (f as unknown as { path?: string }).path;
        if (path) handleOpen(path);
      }
    },
    [handleOpen],
  );

  return (
    <Card
      className={`flex min-h-[360px] cursor-pointer flex-col items-center justify-center border-2 border-dashed p-8 text-center transition-colors md:min-h-[440px] ${
        dragOver ? "border-primary bg-accent/50" : "border-border hover:border-primary/50"
      }`}
      onDragOver={(e) => {
        e.preventDefault();
        setDragOver(true);
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={onDrop}
      onClick={loading ? undefined : onClickPick}
    >
      <div className="flex flex-col items-center justify-center gap-5">
        <div
          className={`rounded-2xl bg-primary/10 p-6 transition-transform ${
            dragOver ? "scale-110" : ""
          }`}
        >
          {loading ? (
            <Loader2 className="h-12 w-12 animate-spin text-primary" />
          ) : (
            <FolderOpen className="h-12 w-12 text-primary" />
          )}
        </div>
        <div className="space-y-1">
          <h2 className="text-lg font-semibold md:text-xl">
            {loading ? "正在解包…" : "打开 MOD 文件"}
          </h2>
          <p className="text-sm text-muted-foreground">
            {loading
              ? "请稍候，正在解析 PAK 内容"
              : "拖拽 .pak 或 .zip 文件到此区域，或点击选择"}
          </p>
        </div>
        {!loading && (
          <Button size="lg" onClick={(e) => { e.stopPropagation(); onClickPick(); }}>
            <FolderOpen className="h-4 w-4" />
            选择文件
          </Button>
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
