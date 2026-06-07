# AeroSync

一个现代化、高性能的 Syncthing 原生客户端。

AeroSync 现在使用 Rust + Slint 构建原生桌面 UI，不再依赖 Tauri、WebView、Vue、Vite 或 npm 前端构建链。

## 开发

```bash
cargo run
```

## 构建

```bash
cargo build
cargo build --release
```

## 说明

- UI：Slint 原生窗口，入口为 `ui/app.slint`。
- 应用入口：`src/main.rs`。
- 控制器：`src/app_controller.rs`。
- Syncthing 后端：`src/backend/syncthing.rs`。
- Syncthing 核心会按需下载到 AeroSync 的应用数据目录，并使用独立配置目录运行。
