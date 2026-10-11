# lolPing 原生桌面版

这是 Rust + GPUI 桌面应用，支持 Windows x64 和 Apple 芯片 Mac。两平台共用轮盘、设置页和输入协议，分别实现悬浮窗、菜单栏或系统托盘，以及音效。

## 开发与打包

完整环境要求、使用说明和验证命令见 [项目 README](../../README.md)。所有命令从仓库根目录运行。

| 平台 | 输入助手 | 启动 | 打包 |
| --- | --- | --- | --- |
| macOS | `sh native/hook-helper/build.sh` | `cargo run --locked -p lolping-native` | `sh native/gpui/dmg.sh` |
| Windows | `./native/hook-helper/build.ps1` | `cargo run --locked -p lolping-native` | `python native/gpui/bundle_windows.py` |

Windows 命令需在 x64 Visual Studio Developer PowerShell 中执行。DMG、安装 EXE 和 ZIP 均输出到 `release/native/`。版本号统一读取 `native/gpui/Cargo.toml`。

## 平台实现

- `src/macos.rs`、`src/macos.m`：AppKit 悬浮窗、菜单栏和 AVFoundation 音效。
- `src/windows.rs`、`src/windows.cpp`：Win32 悬浮窗、系统托盘和 WinMM 音效。
- `src/ui.rs`：共用状态、显示器管理和悬浮层绘制。Windows 全局坐标保持物理像素，绘制及方向判定时按所在显示器缩放换算。
- `windows-installer.iss`：Inno Setup 安装配置，按当前用户安装。

[Native builds 工作流](../../.github/workflows/native.yml) 在推送、Pull Request 和手动运行时生成两平台产物。构建、单元测试、输入协议测试和安装内容检查由 CI 执行；真实输入、多屏及全屏场景需要实机验收。
