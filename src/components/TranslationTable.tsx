import { useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  Check,
  Edit3,
  Languages,
  Loader2,
  Play,
  RotateCcw,
  Search,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Progress } from "@/components/ui/progress";
import { readFileEntries, translateEntries } from "@/lib/tauri";
import { useAppStore } from "@/store/app-store";
import type { TranslationEntry, TranslationStatus } from "@/lib/types";

const STATUS_META: Record<
  TranslationStatus,
  { label: string; variant: "default" | "secondary" | "success" | "warning" | "destructive" }
> = {
  pending: { label: "待翻译", variant: "secondary" },
  translating: { label: "翻译中", variant: "warning" },
  translated: { label: "已翻译", variant: "success" },
  edited: { label: "已编辑", variant: "default" },
  error: { label: "错误", variant: "destructive" },
};

export function TranslationTable() {
  const workDir = useAppStore((s) => s.workDir);
  const selectedFile = useAppStore((s) => s.selectedFile);
  const entries = useAppStore((s) => s.entries);
  const setEntries = useAppStore((s) => s.setEntries);
  const updateEntry = useAppStore((s) => s.updateEntry);
  const setEntryStatus = useAppStore((s) => s.setEntryStatus);
  const appendDelta = useAppStore((s) => s.appendDelta);
  const setError = useAppStore((s) => s.setError);

  const [loading, setLoading] = useState(false);
  const [translating, setTranslating] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [search, setSearch] = useState("");
  const [onlyPending, setOnlyPending] = useState(false);

  // 搜索 + 过滤
  const visibleEntries = useMemo(() => {
    const q = search.trim().toLowerCase();
    return entries.filter((e) => {
      if (onlyPending && e.target) return false;
      if (!q) return true;
      return (
        e.source.toLowerCase().includes(q) ||
        e.target.toLowerCase().includes(q) ||
        e.contentuid.toLowerCase().includes(q)
      );
    });
  }, [entries, search, onlyPending]);

  // 选中文件变化时加载条目
  useEffect(() => {
    if (!workDir || !selectedFile) return;
    setLoading(true);
    setError(null);
    readFileEntries(workDir, selectedFile.name)
      .then((data) => setEntries(data))
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workDir, selectedFile?.name]);

  if (!selectedFile) {
    return (
      <Card className="flex h-full flex-col items-center justify-center p-8 text-center text-muted-foreground">
        <Languages className="mb-3 h-10 w-10" />
        <p>从左侧选择一个本地化文件开始翻译</p>
        <p className="mt-1 text-xs">（标有"本地化 XML/LOCA"或"元数据"的文件）</p>
      </Card>
    );
  }

  // 统计
  const total = entries.length;
  const done = entries.filter(
    (e) => e.status === "translated" || e.status === "edited",
  ).length;

  const onTranslate = async () => {
    if (!workDir) return;
    setTranslating(true);
    setError(null);
    try {
      await translateEntries(workDir, entries, (e) => {
        switch (e.type) {
          case "progress":
            setEntryStatus(e.entryId, e.status);
            break;
          case "delta":
            appendDelta(e.entryId, e.text);
            break;
          case "done":
            updateEntry(e.entryId, {
              target: e.text,
              status: "translated",
            });
            break;
          case "error":
            updateEntry(e.entryId, {
              status: "error",
              error: e.message,
            });
            break;
          case "all_done":
            break;
        }
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setTranslating(false);
    }
  };

  const startEdit = (entry: TranslationEntry) => {
    setEditingId(entry.id);
    setDraft(entry.target);
  };

  const saveEdit = () => {
    if (editingId) {
      updateEntry(editingId, { target: draft, status: "edited" });
      setEditingId(null);
    }
  };

  const revert = (entry: TranslationEntry) => {
    updateEntry(entry.id, { target: "", status: "pending", error: null });
  };

  return (
    <Card className="flex h-full flex-col">
      {/* 工具栏 */}
      <div className="space-y-2 border-b p-3">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold">
              {selectedFile.name}
            </h3>
            <p className="text-xs text-muted-foreground">
              {loading ? "加载中…" : `共 ${total} 条`}
            </p>
          </div>
          <div className="flex items-center gap-2">
            {total > 0 && (
              <div className="flex items-center gap-2">
                <Progress value={done} max={total} className="w-24" />
                <span className="text-xs text-muted-foreground tabular-nums">
                  {done}/{total}
                </span>
              </div>
            )}
            <Button
              size="sm"
              onClick={onTranslate}
              disabled={translating || loading || total === 0}
            >
              {translating ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  翻译中…
                </>
              ) : (
                <>
                  <Play className="h-4 w-4" />
                  翻译全部
                </>
              )}
            </Button>
          </div>
        </div>
        {total > 0 && (
          <div className="flex items-center gap-2">
            <div className="relative flex-1">
              <Search className="absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                placeholder="搜索原文、译文或 contentuid…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="h-8 pl-8 text-xs"
              />
            </div>
            <Button
              size="sm"
              variant={onlyPending ? "default" : "outline"}
              className="h-8 text-xs"
              onClick={() => setOnlyPending(!onlyPending)}
            >
              {onlyPending ? "✓ " : ""}仅待翻译
            </Button>
          </div>
        )}
      </div>

      {/* 条目列表 */}
      <div className="flex-1 overflow-auto">
        {loading ? (
          <div className="flex h-full items-center justify-center text-muted-foreground">
            <Loader2 className="mr-2 h-5 w-5 animate-spin" />
            加载条目…
          </div>
        ) : entries.length === 0 ? (
          <div className="flex h-full items-center justify-center p-8 text-center text-sm text-muted-foreground">
            该文件没有可翻译条目
          </div>
        ) : visibleEntries.length === 0 ? (
          <div className="flex h-full items-center justify-center p-8 text-center text-sm text-muted-foreground">
            没有匹配的条目
          </div>
        ) : (
          <div className="divide-y">
            {visibleEntries.map((entry) => (
              <div
                key={entry.id}
                className={cn(
                  "grid grid-cols-1 gap-3 p-3 md:grid-cols-2",
                  entry.status === "translating" && "bg-amber-500/5",
                  entry.status === "error" && "bg-destructive/5",
                )}
              >
                {/* 原文 */}
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[10px] text-muted-foreground">
                      原文
                    </span>
                    <Badge
                      variant={STATUS_META[entry.status].variant}
                      className="text-[10px]"
                    >
                      {entry.status === "translating" && (
                        <Loader2 className="mr-1 h-2.5 w-2.5 animate-spin" />
                      )}
                      {entry.status === "error" && (
                        <AlertCircle className="mr-1 h-2.5 w-2.5" />
                      )}
                      {STATUS_META[entry.status].label}
                    </Badge>
                  </div>
                  <p className="rounded bg-muted/50 p-2 text-sm">
                    {entry.source}
                  </p>
                </div>

                {/* 译文 */}
                <div className="space-y-1">
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] text-muted-foreground">
                      译文
                    </span>
                    <div className="flex items-center gap-1">
                      {editingId === entry.id ? (
                        <>
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-6 px-2 text-xs"
                            onClick={() => setEditingId(null)}
                          >
                            取消
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            className="h-6 px-2 text-xs"
                            onClick={saveEdit}
                          >
                            <Check className="h-3 w-3" />
                            保存
                          </Button>
                        </>
                      ) : (
                        <>
                          {entry.target && (
                            <Button
                              size="sm"
                              variant="ghost"
                              className="h-6 px-2 text-xs"
                              onClick={() => startEdit(entry)}
                            >
                              <Edit3 className="h-3 w-3" />
                              编辑
                            </Button>
                          )}
                          {(entry.status === "translated" ||
                            entry.status === "edited" ||
                            entry.status === "error") && (
                            <Button
                              size="sm"
                              variant="ghost"
                              className="h-6 px-2 text-xs"
                              onClick={() => revert(entry)}
                            >
                              <RotateCcw className="h-3 w-3" />
                              还原
                            </Button>
                          )}
                        </>
                      )}
                    </div>
                  </div>
                  {editingId === entry.id ? (
                    <Textarea
                      value={draft}
                      onChange={(e) => setDraft(e.target.value)}
                      className="min-h-[60px] text-sm"
                      autoFocus
                    />
                  ) : (
                    <p
                      className={cn(
                        "min-h-[36px] rounded border border-transparent p-2 text-sm",
                        entry.target ? "bg-primary/5" : "bg-muted/30 text-muted-foreground italic",
                        entry.error && "text-destructive",
                      )}
                    >
                      {entry.error ?? entry.target ?? "等待翻译…"}
                    </p>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </Card>
  );
}
