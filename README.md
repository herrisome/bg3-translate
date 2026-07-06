# BG3 MOD 汉化工具 ✨

一个给《博德之门 3》MOD 用的桌面汉化工具。

它不追求花哨，核心就是三件事：打开 MOD、翻译文本、重新打包。

## 能做什么

- 📦 打开 `.pak` 或常见 `.zip` MOD 包
- 🧩 自动找出 XML、LOCA、LSX 里的可翻译内容
- 🌏 调用 OpenAI 兼容接口，支持 DeepSeek、智谱、Kimi、本地 Ollama 等模型
- 📚 带术语表，尽量贴近 BG3 和 D&D 的官方译名
- ✍️ 翻译后可以人工改，觉得不顺手就直接修
- 🚫 翻译过程中可以取消，不用硬等
- 🧷 同一个 MOD 内尽量保持译名一致
- 🛠️ 支持只解压，也支持重新打包为 BG3 可加载的 `.pak`

## 下载

到 Releases 下载：

- Windows x64（zh-CN）：MSI 安装包
- Windows x64（zh-CN）：EXE 安装程序
- Windows x64（zh-CN）：便携版

便携版解压后直接运行 `bg3-translate.exe`。

## 使用

1. 打开软件，选择 `.pak` 或 `.zip`。
2. 在设置里填好大模型接口。
3. 选中要翻译的文件，开始翻译。
4. 翻完后检查几条重点文本。
5. 保存并重新打包。

如果某个 MOD 有特殊语境，比如“XX 是姿势名称”，可以在翻译界面里填一条提示，模型会按这个方向处理。

## 配置放在哪里

配置文件和术语表默认放在 exe 同级目录，方便便携使用和备份。

## 开发

```bash
bun install
bun tauri dev
```

构建：

```bash
bun tauri build
```

常用检查：

```bash
bun run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo check --manifest-path src-tauri/Cargo.toml
```

## 技术栈

- Tauri 2
- React 18
- TypeScript
- Tailwind CSS
- shadcn/ui
- Rust

## 备注

这个工具主要面向 BG3 MOD 汉化。它会尽量保留标签、占位符、contentuid 和 version，但打包前最好还是抽几条关键文本进游戏里看一眼。
