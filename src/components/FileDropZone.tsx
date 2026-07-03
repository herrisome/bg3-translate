import { useCallback, useRef, useState } from "react";
import { FileArchive, FolderOpen, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
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
      className={`border-2 border-dashed transition-colors ${
        dragOver ? "border-primary bg-accent/50" : "border-border"
      }`}
      onDragOver={(e) => {
        e.preventDefault();
        setDragOver(true);
      }}
      onDragLeave={() => setDragOver(false)}
      onDrop={onDrop}
    >
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <FileArchive className="h-5 w-5" />
          选择 MOD 文件
        </CardTitle>
        <CardDescription>
          支持 .pak 或 .zip（Nexus 标准打包）格式的博德之门3 MOD
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="flex flex-col items-center justify-center gap-4 py-8">
          <div className="rounded-full bg-muted p-4">
            <FolderOpen className="h-8 w-8 text-muted-foreground" />
          </div>
          <p className="text-sm text-muted-foreground">
            拖拽文件到此处，或点击下方按钮选择
          </p>
          <Button
            onClick={onClickPick}
            disabled={loading}
            size="lg"
            ref={undefined}
          >
            {loading ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                解包中…
              </>
            ) : (
              <>
                <FolderOpen className="h-4 w-4" />
                选择 MOD 文件
              </>
            )}
          </Button>
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
          <p className="max-w-md text-center text-xs text-muted-foreground">
            提示：samples 目录下有测试用 MOD。所有处理在本地完成，不会上传你的文件。
          </p>
        </div>
      </CardContent>
    </Card>
  );
}
