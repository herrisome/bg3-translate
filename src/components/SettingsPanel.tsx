import { useEffect, useState } from "react";
import { Loader2, Save, Settings2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { loadLlmSettings, saveLlmSettings } from "@/lib/tauri";
import { useAppStore } from "@/store/app-store";
import type { LlmSettings } from "@/lib/types";

export function SettingsPanel({ compact = false }: { compact?: boolean }) {
  const settings = useAppStore((s) => s.settings);
  const setSettings = useAppStore((s) => s.setSettings);
  const [form, setForm] = useState<LlmSettings>(settings);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  // 首次加载已保存设置
  useEffect(() => {
    loadLlmSettings()
      .then((s) => {
        setForm(s);
        setSettings(s);
      })
      .catch(() => {
        /* 用默认值 */
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onSave = async () => {
    setSaving(true);
    try {
      await saveLlmSettings(form);
      setSettings(form);
      setSaved(true);
      setTimeout(() => setSaved(false), 1500);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Card className={compact ? "border-0 shadow-none" : ""}>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Settings2 className="h-4 w-4" />
          大模型配置
        </CardTitle>
        <CardDescription>
          使用 OpenAI 兼容协议，支持 DeepSeek、智谱、Kimi、OpenAI、本地 Ollama 等
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="space-y-2">
          <Label htmlFor="baseUrl">API Base URL</Label>
          <Input
            id="baseUrl"
            placeholder="https://api.deepseek.com"
            value={form.baseUrl}
            onChange={(e) => setForm({ ...form, baseUrl: e.target.value })}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="apiKey">API Key</Label>
          <Input
            id="apiKey"
            type="password"
            placeholder="sk-..."
            value={form.apiKey}
            onChange={(e) => setForm({ ...form, apiKey: e.target.value })}
          />
          <p className="text-xs text-muted-foreground">
            密钥仅保存在本地配置文件，不会上传。
          </p>
        </div>
        <div className="space-y-2">
          <Label htmlFor="model">模型名称</Label>
          <Input
            id="model"
            placeholder="deepseek-chat"
            value={form.model}
            onChange={(e) => setForm({ ...form, model: e.target.value })}
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="concurrency">并发数（同时翻译的条目数）</Label>
          <Input
            id="concurrency"
            type="number"
            min={1}
            max={16}
            value={form.concurrency}
            onChange={(e) =>
              setForm({ ...form, concurrency: Number(e.target.value) })
            }
          />
          <p className="text-xs text-muted-foreground">
            数值越大翻译越快，但会增加 API 并发请求量。建议 4-8。
          </p>
        </div>
        <Button onClick={onSave} disabled={saving} className="w-full">
          {saving ? (
            <>
              <Loader2 className="h-4 w-4 animate-spin" /> 保存中…
            </>
          ) : saved ? (
            "✓ 已保存"
          ) : (
            <>
              <Save className="h-4 w-4" /> 保存配置
            </>
          )}
        </Button>
      </CardContent>
    </Card>
  );
}
