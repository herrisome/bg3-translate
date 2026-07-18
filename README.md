<img width="1800" height="1200" alt="image" src="https://github.com/user-attachments/assets/ab92cbdf-e136-49ac-aaa5-5ba8c804b93f" /># BG3 MOD 汉化工具 ✨

一个给《博德之门 3》MOD 用的桌面汉化工具，核心就是三件事：打开 MOD、翻译文本、重新打包。

## 下载

到 Releases 下载：

- Windows x64（zh-CN）：MSI 安装包
- Windows x64（zh-CN）：EXE 安装程序
- Windows x64（zh-CN）：便携版

便携版解压后直接运行 `bg3-translate.exe`。

## 使用

1. 打开软件，选择 `.pak` 或 `.zip`。
<img width="1800" height="1200" alt="image" src="https://github.com/user-attachments/assets/c9ebb1b8-dfad-4b1b-bb70-73dc7978b92f" />
3. 在设置里填好大模型接口。
<img width="1800" height="1200" alt="image" src="https://github.com/user-attachments/assets/158aa36a-b31f-4736-98a0-d90273c21326" />
5. 选中要翻译的文件，开始翻译。
<img width="1800" height="1200" alt="image" src="https://github.com/user-attachments/assets/08986436-d270-40fc-962e-677dd48a8fda" />
7. 翻完后检查几条重点文本。
8. 保存并重新打包。
<img width="1800" height="1200" alt="image" src="https://github.com/user-attachments/assets/f37730ec-e661-47ea-bc21-174d9b0a13b9" />


如果某个 MOD 有特殊语境，可以在翻译界面里填一条提示，模型会按这个方向处理。
<img width="1800" height="322" alt="image" src="https://github.com/user-attachments/assets/d1755cc9-6345-4ee3-b74f-1f7ee831c842" />

## 配置放在哪里

配置文件和术语表默认放在 exe 同级目录，方便便携使用和备份。
<img width="1800" height="1200" alt="image" src="https://github.com/user-attachments/assets/db88d79d-bc31-4500-997e-c370a4ee90b6" />

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
