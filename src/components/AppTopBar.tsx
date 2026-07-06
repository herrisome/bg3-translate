import {
  BookOpen,
  CheckCircle2,
  ChevronRight,
  Languages,
  Minus,
  Package,
  Palette,
  Settings,
  Square,
  X,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useAppStore } from "@/store/app-store";
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
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          size="icon"
          variant="outline"
          className="h-9 w-9"
          title="切换主题"
          aria-label="切换主题"
        >
          <Palette className="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-36">
        <DropdownMenuRadioGroup
          value={theme}
          onValueChange={(value) => setTheme(value as Theme)}
        >
          {THEMES.map((t) => (
            <DropdownMenuRadioItem key={t.value} value={t.value}>
              {t.label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function WindowControls() {
  const win = getCurrentWindow();

  return (
    <div className="ml-1 flex items-center">
      <button
        type="button"
        className="flex h-[54px] w-11 items-center justify-center text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        onClick={() => void win.minimize()}
        title="最小化"
        aria-label="最小化"
      >
        <Minus className="h-4 w-4" />
      </button>
      <button
        type="button"
        className="flex h-[54px] w-11 items-center justify-center text-muted-foreground hover:bg-accent hover:text-accent-foreground"
        onClick={() => void win.toggleMaximize()}
        title="最大化"
        aria-label="最大化"
      >
        <Square className="h-3.5 w-3.5" />
      </button>
      <button
        type="button"
        className="flex h-[54px] w-11 items-center justify-center text-muted-foreground hover:bg-destructive hover:text-destructive-foreground"
        onClick={() => void win.close()}
        title="关闭窗口"
        aria-label="关闭窗口"
      >
        <X className="h-4 w-4" />
      </button>
    </div>
  );
}

export function AppTopBar({
  title = "BG3 MOD 汉化工具",
  subtitle = "博德之门3 MOD 本地化翻译",
  showStepper = false,
  onOpenGlossary,
  onOpenSettings,
  onClose,
  actions,
}: {
  title?: string;
  subtitle?: string;
  showStepper?: boolean;
  onOpenGlossary?: () => void;
  onOpenSettings?: () => void;
  onClose?: () => void;
  actions?: React.ReactNode;
}) {
  const configured = useAppStore((s) => s.settings.apiKey.length > 0);
  const win = getCurrentWindow();

  const startDragging = (event: React.MouseEvent) => {
    if (event.button !== 0) return;
    void win.startDragging();
  };

  const toggleMaximize = () => {
    void win.toggleMaximize();
  };

  return (
    <header
      data-tauri-drag-region
      className="relative flex h-[54px] shrink-0 select-none items-center border-b pl-4 md:pl-6"
    >
      <div
        data-tauri-drag-region
        className="z-10 flex min-w-0 flex-1 items-center gap-2 self-stretch pr-4"
        onMouseDown={startDragging}
        onDoubleClick={toggleMaximize}
      >
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
          <Languages className="h-4 w-4" />
        </div>
        <div data-tauri-drag-region className="min-w-0">
          <h1 className="truncate text-sm font-bold leading-tight">{title}</h1>
          <p className="truncate text-[10px] text-muted-foreground">{subtitle}</p>
        </div>
      </div>

      {showStepper && (
        <div
          data-tauri-drag-region
          className="pointer-events-none absolute left-1/2 top-1/2 hidden -translate-x-1/2 -translate-y-1/2 lg:flex"
        >
          <Stepper />
        </div>
      )}

      <div className="z-10 ml-auto flex min-w-0 items-center gap-2 md:gap-3">
        {actions}
        <ThemeSwitcher />
        {onOpenSettings && (
          <Button
            size="icon"
            variant="outline"
            className="relative h-9 w-9"
            onClick={onOpenSettings}
            title="设置"
            aria-label="设置"
          >
            <Settings className="h-4 w-4" />
            <span
              className={`absolute right-1.5 top-1.5 h-2 w-2 rounded-full ${
                configured ? "bg-emerald-500" : "bg-amber-500"
              }`}
            />
          </Button>
        )}
        {onOpenGlossary && (
          <Button
            size="icon"
            variant="outline"
            className="h-9 w-9"
            onClick={onOpenGlossary}
            title="术语表"
            aria-label="术语表"
          >
            <BookOpen className="h-4 w-4" />
          </Button>
        )}
        {onClose && (
          <Button
            size="sm"
            variant="ghost"
            className="h-9"
            onClick={onClose}
          >
            关闭
          </Button>
        )}
        <WindowControls />
      </div>
    </header>
  );
}
