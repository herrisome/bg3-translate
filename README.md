# BG3 MOD 汉化工具

将博德之门 3 的 MOD（.pak/.zip）通过大模型翻译为简体中文，翻译风格符合 D&D 与博德之门 3 的世界观和语境。

## 功能

- 选择 MOD 文件（.pak 或 Nexus 标准 .zip 打包），自动解包
- 识别 PAK 内可翻译文件：本地化 XML（`<contentList>`）、二进制 `.loca`、LSX 元数据
- 调用大模型（OpenAI 兼容协议：DeepSeek / 智谱 / Kimi / OpenAI / 本地 Ollama 等）流式翻译
- D&D 5e + BG3 官方译名语境，保留 `<LSTag>` 富文本标签和 `{1}` 占位符
- 翻译过程实时显示，可逐条人工校对编辑
- 原样重新打包为 BG3 可加载的 `.pak`（保持 LSPK v18 格式）

## 工作流程

```
选择 .pak/.zip → 解包 → 识别可翻译文件 → LLM 流式翻译 → 人工校对 → 重新打包 .pak
```

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri v2 |
| 前端 | React 18 + TypeScript + Vite |
| UI | shadcn/ui + Tailwind CSS + Lucide |
| 状态管理 | Zustand |
| PAK 处理 | bg3rustpaklib（纯 Rust，支持 LSPK v18 + LOCA 互转） |
| LLM 调用 | reqwest + 手动 SSE 解析 + Tauri Channel |
| 格式解析 | quick-xml |

## 开发

### 环境要求

- Node.js 20+ / bun
- Rust 1.77+（stable）
- macOS / Linux / Windows

### 启动开发模式

```bash
bun install
bun tauri dev
```

### 构建

```bash
bun tauri build
```

产物在 `src-tauri/target/release/bundle/` 下。

### 测试

```bash
# Rust 单元 + 端到端测试（用 samples/ 下的真实 MOD）
cargo test --manifest-path src-tauri/Cargo.toml

# 前端构建检查
bun run build
```

## 项目结构

```
bg3-translate/
├── src/                    # React 前端
│   ├── components/         # UI 组件（含 shadcn/ui 基础组件）
│   ├── lib/                # Tauri 调用封装、类型定义
│   ├── store/              # Zustand 状态
│   └── App.tsx             # 主应用（首页/翻译/打包 三阶段）
├── src-tauri/
│   └── src/
│       ├── pak.rs          # PAK 解包/打包/文件识别
│       ├── formats.rs      # XML/LSX/LOCA ↔ TranslationEntry 互转
│       ├── translation.rs  # LLM 翻译引擎（流式 + 批量并发）
│       ├── commands.rs     # Tauri 命令入口
│       └── config.rs       # 设置持久化
└── samples/                # 测试用 MOD
```

## 翻译正确性保证

以下规则在代码中强制执行（已通过真实 MOD 验证）：

- **contentuid 和 version 绝不修改**：只翻译文本内容，句柄原样保留
- **富文本标签保留**：`<LSTag Tag="...">`、`<font>`、`<i>` 等标签完整保留
- **占位符保留**：`{1}`、`{2}` 等参数占位符的数量与位置不变
- **LSPK 版本一致**：重打包保持 v18 格式，确保游戏可加载
- **LOCA 往返无损**：二进制本地化文件经 XML 中间表示翻译后，字节级可还原

## LLM 配置

在应用首页的"大模型配置"面板填写：

- **API Base URL**：如 `https://api.deepseek.com`、`https://open.bigmodel.cn/api/paas/v4`
- **API Key**：你的密钥（仅存本地配置文件）
- **模型名称**：如 `deepseek-chat`、`glm-4-flash`
- **并发数 / 每批条目数**：影响翻译速度与上下文一致性

设置保存在 `~/.config/bg3-translate/settings.json`（macOS/Linux）或 `%APPDATA%/bg3-translate/settings.json`（Windows）。
