import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
import {
  Check,
  Edit3,
  Loader2,
  Plus,
  RotateCcw,
  Search,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import {
  addGlossaryEntry,
  deleteGlossaryEntry,
  importGlossary,
  listGlossary,
  resetGlossary,
  updateGlossaryEntry,
} from "@/lib/tauri";
import { AppTopBar } from "@/components/AppTopBar";
import type { Glossary, GlossaryEntry } from "@/lib/types";

function newEmptyEntry(): GlossaryEntry {
  return {
    source: "",
    target: "",
    sourceKind: "user",
    enabled: true,
    ambiguous: false,
    wholeWord: true,
    caseSensitive: false,
    count: 0,
  };
}

export function GlossaryPanel({
  onClose,
  embedded = false,
}: {
  onClose: () => void;
  embedded?: boolean;
}) {
  const [glossary, setGlossary] = useState<Glossary | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<GlossaryEntry | null>(null);
  const [editOriginal, setEditOriginal] = useState<string | null>(null);
  const [limit, setLimit] = useState(200);

  useEffect(() => {
    refresh();
  }, []);

  const refresh = async () => {
    setLoading(true);
    setError(null);
    try {
      const g = await listGlossary();
      setGlossary(g);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  // 过滤 + 搜索（只渲染前 limit 条，避免 20K 条卡顿）
  const visible = useMemo(() => {
    if (!glossary) return [];
    const q = search.trim().toLowerCase();
    return glossary.terms.filter(
      (t) =>
        !q ||
        t.source.toLowerCase().includes(q) ||
        t.target.toLowerCase().includes(q),
    );
  }, [glossary, search]);

  const onSave = async () => {
    if (!editing) return;
    if (!editing.source.trim() || !editing.target.trim()) {
      setError("术语的中英文均不能为空");
      return;
    }
    setError(null);
    try {
      let g: Glossary;
      if (editOriginal) {
        g = await updateGlossaryEntry(editOriginal, editing);
      } else {
        g = await addGlossaryEntry(editing);
      }
      setGlossary(g);
      setEditing(null);
      setEditOriginal(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const onDelete = async (source: string) => {
    setError(null);
    try {
      const g = await deleteGlossaryEntry(source);
      setGlossary(g);
    } catch (e) {
      setError(String(e));
    }
  };

  const onReset = async () => {
    if (!confirm("确定要重置术语表为内置官方种子吗？所有用户自定义将被清除。")) return;
    setError(null);
    try {
      const g = await resetGlossary();
      setGlossary(g);
    } catch (e) {
      setError(String(e));
    }
  };

  const onImport = async () => {
    // 用 Tauri 原生对话框（浏览器 <input> 在 Tauri 沙箱里读不到真实文件内容）
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "术语表 JSON", extensions: ["json"] }],
    });
    const filePath = typeof selected === "string" ? selected : null;
    if (!filePath) return;

    setError(null);
    setLoading(true);
    try {
      const text = await readTextFile(filePath);
      const g = await importGlossary(text);
      setGlossary(g);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const startEdit = (entry: GlossaryEntry) => {
    setEditing({ ...entry });
    setEditOriginal(entry.source);
  };

  const startAdd = () => {
    setEditing(newEmptyEntry());
    setEditOriginal(null);
  };

  return (
    <div className="flex h-full flex-col">
      {!embedded && (
        <AppTopBar
          title="术语表"
          subtitle={
            glossary
              ? `${glossary.terms.length} 条术语${
                  visible.length !== glossary.terms.length
                    ? `，当前显示 ${visible.length}`
                    : ""
                }`
              : "加载中..."
          }
          onClose={onClose}
          actions={
            <div className="flex items-center gap-2">
              <Button size="sm" variant="outline" onClick={onImport}>
                <Upload className="h-4 w-4" />
                <span className="hidden sm:inline">导入</span>
              </Button>
              <Button size="sm" variant="outline" onClick={onReset}>
                <RotateCcw className="h-4 w-4" />
                <span className="hidden sm:inline">重置</span>
              </Button>
              <Button size="sm" onClick={startAdd}>
                <Plus className="h-4 w-4" />
                <span className="hidden sm:inline">新增</span>
              </Button>
            </div>
          }
        />
      )}
      {embedded && (
        <div className="flex shrink-0 items-center justify-between gap-2 border-b px-4 py-2">
          <div className="min-w-0 text-xs text-muted-foreground">
            {glossary
              ? `${glossary.terms.length} 条术语${
                  visible.length !== glossary.terms.length
                    ? `，当前显示 ${visible.length}`
                    : ""
                }`
              : "加载中..."}
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Button size="sm" variant="outline" onClick={onImport}>
              <Upload className="h-4 w-4" />
              <span className="hidden sm:inline">导入</span>
            </Button>
            <Button size="sm" variant="outline" onClick={onReset}>
              <RotateCcw className="h-4 w-4" />
              <span className="hidden sm:inline">重置</span>
            </Button>
            <Button size="sm" onClick={startAdd}>
              <Plus className="h-4 w-4" />
              <span className="hidden sm:inline">新增</span>
            </Button>
          </div>
        </div>
      )}
      {error && (
        <div className="bg-destructive px-4 py-2 text-sm text-destructive-foreground">
          ⚠ {error}
        </div>
      )}

      {/* 搜索 */}
      <div className="flex flex-wrap items-center gap-2 border-b px-4 py-2">
        <div className="relative flex-1">
          <Search className="absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder="搜索术语（英文或中文）…"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setLimit(200);
            }}
            className="h-8 pl-8 text-xs"
          />
        </div>
      </div>

      {/* 列表 */}
      <div className="min-h-0 flex-1 overflow-auto">
        {loading ? (
          <div className="flex h-full items-center justify-center text-muted-foreground">
            <Loader2 className="mr-2 h-5 w-5 animate-spin" />
            加载术语表…
          </div>
        ) : visible.length === 0 ? (
          <div className="flex h-full items-center justify-center p-8 text-center text-sm text-muted-foreground">
            没有匹配的术语
          </div>
        ) : (
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-muted/80 backdrop-blur">
              <tr className="text-left text-xs text-muted-foreground">
                <th className="px-4 py-2 font-medium">英文 (source)</th>
                <th className="px-4 py-2 font-medium">中文 (target)</th>
                <th className="px-4 py-2 font-medium">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y">
              {visible.slice(0, limit).map((t) => (
                <tr
                  key={t.source}
                  className={cn(
                    "hover:bg-accent/50",
                    !t.enabled &&
                      "bg-muted/30 text-muted-foreground opacity-60 hover:bg-muted/40",
                  )}
                >
                  <td className="max-w-[180px] truncate px-4 py-1.5 sm:max-w-[280px]" title={t.source}>
                    {t.source}
                  </td>
                  <td className="max-w-[180px] truncate px-4 py-1.5 sm:max-w-[280px]" title={t.target}>
                    {t.target}
                  </td>
                  <td className="px-4 py-1.5">
                    <div className="flex gap-1">
                      <Button
                        size="sm"
                        variant="ghost"
                        className="h-6 px-1.5"
                        onClick={() => startEdit(t)}
                      >
                        <Edit3 className="h-3 w-3" />
                      </Button>
                      {t.sourceKind !== "official" && (
                        <Button
                          size="sm"
                          variant="ghost"
                          className="h-6 px-1.5 text-destructive hover:text-destructive"
                          onClick={() => onDelete(t.source)}
                        >
                          <Trash2 className="h-3 w-3" />
                        </Button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {visible.length > limit && (
          <div className="border-t p-3 text-center">
            <Button variant="outline" size="sm" onClick={() => setLimit(limit + 200)}>
              加载更多（剩余 {visible.length - limit} 条）
            </Button>
          </div>
        )}
      </div>

      {/* 编辑弹层 */}
      {editing && (
        <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
          <Card className="w-full max-w-md p-5">
            <div className="mb-4 flex items-center justify-between">
              <h3 className="text-sm font-semibold">
                {editOriginal ? "编辑术语" : "新增术语"}
              </h3>
              <Button
                size="sm"
                variant="ghost"
                className="h-7 w-7 p-0"
                onClick={() => {
                  setEditing(null);
                  setEditOriginal(null);
                }}
              >
                <X className="h-4 w-4" />
              </Button>
            </div>
            <div className="space-y-3">
              <div className="space-y-1">
                <Label className="text-xs">英文 (source)</Label>
                <Textarea
                  value={editing.source}
                  onChange={(e) => setEditing({ ...editing, source: e.target.value })}
                  className="min-h-[40px] text-sm"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-xs">中文 (target)</Label>
                <Textarea
                  value={editing.target}
                  onChange={(e) => setEditing({ ...editing, target: e.target.value })}
                  className="min-h-[40px] text-sm"
                />
              </div>
              <div className="space-y-1">
                <Label className="text-xs">选项</Label>
                <div className="flex flex-wrap gap-4 pt-1 text-xs">
                  <label className="flex items-center gap-1.5">
                    <input
                      type="checkbox"
                      checked={editing.enabled}
                      onChange={(e) =>
                        setEditing({ ...editing, enabled: e.target.checked })
                      }
                    />
                    启用
                  </label>
                  <label className="flex items-center gap-1.5">
                    <input
                      type="checkbox"
                      checked={editing.wholeWord}
                      onChange={(e) =>
                        setEditing({ ...editing, wholeWord: e.target.checked })
                      }
                    />
                    整词匹配
                  </label>
                </div>
              </div>
            </div>
            <div className="mt-4 flex justify-end gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  setEditing(null);
                  setEditOriginal(null);
                }}
              >
                取消
              </Button>
              <Button size="sm" onClick={onSave}>
                <Check className="h-4 w-4" />
                保存
              </Button>
            </div>
          </Card>
        </div>
      )}
    </div>
  );
}
