import { useMemo, useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  FileCode,
  FileText,
  FileType2,
  Languages,
  Package,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import type { PakFile, PakFileKind } from "@/lib/types";

interface TreeNode {
  name: string;
  path: string;
  file?: PakFile;
  children: Map<string, TreeNode>;
}

const KIND_ICON: Record<PakFileKind, React.ReactNode> = {
  "localization-xml": <Languages className="h-4 w-4 text-blue-500" />,
  "localization-loca": <Languages className="h-4 w-4 text-indigo-500" />,
  "metadata-lsx": <FileType2 className="h-4 w-4 text-amber-500" />,
  "script-lua": <FileCode className="h-4 w-4 text-emerald-500" />,
  "data-txt": <FileText className="h-4 w-4 text-muted-foreground" />,
  other: <FileText className="h-4 w-4 text-muted-foreground" />,
};

const KIND_LABEL: Record<PakFileKind, string> = {
  "localization-xml": "本地化 XML",
  "localization-loca": "本地化 LOCA",
  "metadata-lsx": "元数据",
  "script-lua": "Lua 脚本",
  "data-txt": "数据",
  other: "其他",
};

const TRANSLATABLE: PakFileKind[] = [
  "localization-xml",
  "localization-loca",
  "metadata-lsx",
];

function buildTree(files: PakFile[]): TreeNode {
  const root: TreeNode = {
    name: "",
    path: "",
    children: new Map(),
  };
  for (const file of files) {
    const parts = file.name.split("/").filter(Boolean);
    let cur = root;
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      const isLeaf = i === parts.length - 1;
      if (!cur.children.has(part)) {
        cur.children.set(part, {
          name: part,
          path: parts.slice(0, i + 1).join("/"),
          file: isLeaf ? file : undefined,
          children: new Map(),
        });
      }
      cur = cur.children.get(part)!;
    }
  }
  return root;
}

function TreeRow({
  node,
  depth,
  selectedPath,
  onSelect,
}: {
  node: TreeNode;
  depth: number;
  selectedPath: string | null;
  onSelect: (file: PakFile) => void;
}) {
  const [expanded, setExpanded] = useState(depth < 1);
  const isFolder = node.children.size > 0;

  if (isFolder) {
    const childFiles = [...node.children.values()];
    childFiles.sort((a, b) => {
      const aFolder = a.children.size > 0 ? 0 : 1;
      const bFolder = b.children.size > 0 ? 0 : 1;
      return aFolder - bFolder || a.name.localeCompare(b.name);
    });
    return (
      <div>
        <button
          className="flex w-full items-center gap-1 rounded px-2 py-1 text-sm hover:bg-accent"
          style={{ paddingLeft: depth * 16 + 8 }}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? (
            <ChevronDown className="h-3.5 w-3.5 shrink-0" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 shrink-0" />
          )}
          <Package className="h-4 w-4 shrink-0 text-muted-foreground" />
          <span className="truncate">{node.name}</span>
        </button>
        {expanded &&
          childFiles.map((child) => (
            <TreeRow
              key={child.path}
              node={child}
              depth={depth + 1}
              selectedPath={selectedPath}
              onSelect={onSelect}
            />
          ))}
      </div>
    );
  }

  const file = node.file!;
  return (
    <button
      className={cn(
        "flex w-full items-center gap-2 rounded px-2 py-1 text-sm hover:bg-accent",
        selectedPath === file.name && "bg-accent",
      )}
      style={{ paddingLeft: depth * 16 + 24 }}
      onClick={() => onSelect(file)}
    >
      {KIND_ICON[file.kind]}
      <span className="flex-1 truncate text-left">{node.name}</span>
      <Badge variant="secondary" className="shrink-0 text-[10px]">
        {KIND_LABEL[file.kind]}
      </Badge>
      {file.language && (
        <span className="shrink-0 text-[10px] text-muted-foreground">
          {file.language}
        </span>
      )}
    </button>
  );
}

export function FileTree({
  files,
  selectedFile,
  onSelect,
}: {
  files: PakFile[];
  selectedFile: PakFile | null;
  onSelect: (file: PakFile) => void;
}) {
  // 只展示可翻译的文件（隐藏 Lua 脚本、stats 数据等无需翻译的内容）
  const translatableFiles = useMemo(
    () => files.filter((f) => TRANSLATABLE.includes(f.kind)),
    [files],
  );
  const tree = useMemo(() => buildTree(translatableFiles), [translatableFiles]);

  return (
    <Card className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b p-3">
        <div>
          <h3 className="text-sm font-semibold">可翻译文件</h3>
          <p className="text-xs text-muted-foreground">
            {translatableFiles.length} 个文件待翻译
          </p>
        </div>
      </div>
      <div className="flex-1 overflow-auto p-2">
        {translatableFiles.length === 0 ? (
          <div className="flex h-full items-center justify-center p-4 text-center text-sm text-muted-foreground">
            此 MOD 没有可翻译的文件
          </div>
        ) : (
          [...tree.children.values()].map((node) => (
            <TreeRow
              key={node.path}
              node={node}
              depth={0}
              selectedPath={selectedFile?.name ?? null}
              onSelect={onSelect}
            />
          ))
        )}
      </div>
    </Card>
  );
}

export { TRANSLATABLE };
