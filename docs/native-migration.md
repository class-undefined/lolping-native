# 原生迁移与包体积分析

## 上游项目

以下分析针对迁移前的 [上游项目](https://github.com/darrenprx/lolPing/tree/86a0c4696d3d716d3da05f4e08a05587a7b9b651)。文中的 `src/`、`package.json` 和 `tools/pruneAssets.ts` 路径属于上游，未随原生版迁入。

原桌面程序使用 Electron 44.5.1。`package.json` 的入口为 `out/main/index.js`，开发和打包分别使用 `electron-vite` 与 `electron-builder`。设置页使用 React 19 和 Fluent UI，信号层使用 DOM、SVG、CSS 动画和 Web Audio。

`src/main/overlayManager.ts` 为每个显示器创建透明、鼠标穿透、始终置顶的 `BrowserWindow`。`native/hook-helper` 是独立 C++ 输入进程，通过 JSON Lines 与主进程通信。该输入进程不依赖 Electron，因此第一阶段可直接复用。

## 体积来源

以下数字来自仓库文件的逻辑大小，使用 MiB，即 1,048,576 字节。当前工作区最初没有已构建的 Electron 安装包，因此无法逐项复算用户看到的 100 多 MB 产物。

| 内容 | 原始文件大小 |
| --- | ---: |
| 表情动画 WebP | 47.72 MiB |
| 表情 PNG 与 OGG | 1.46 MiB |
| 所有信号 WAV | 7.27 MiB |
| 所有信号 PNG | 0.53 MiB |
| 原生首版实际需要的 29 个信号素材 | 3.96 MiB |

Electron 安装包还包含 Chromium、Node.js 和框架运行库。素材和运行时都需要计入体积，不能把全部大小归因于 JavaScript 代码。

现有 `tools/pruneAssets.ts` 已剔除未使用的素材，打包范围也只包含 `out/**/*` 与 `package.json`。继续清理开发文件的收益有限。表情动画本身接近 48 MiB，即使更换 UI 框架，完整保留这些资源也会带来相应的大小。

首版 `.app` 和 ZIP 的实际大小由 `python3 native/gpui/bundle.py` 在打包结束时报告。`.app` 为未压缩安装内容，ZIP 为压缩下载文件；两者应分别与相同口径的旧版产物比较。首版暂不包含表情，因此体积变化同时来自框架替换和功能范围调整。

2026-10-11 在本机完成 Apple 芯片版本构建，结果如下：

| 原生产物 | 实测大小 |
| --- | ---: |
| Rust 主程序 | 5.76 MiB |
| C++ 输入助手 | 0.11 MiB |
| 信号素材 | 3.96 MiB |
| 完整 `.app`，含图标和签名 | 10.41 MiB |
| ZIP 下载包 | 5.90 MiB |

## 实现结构

| 模块 | 责任 |
| --- | --- |
| `native/gpui/src/catalog.rs` | 信号定义与打包素材清单 |
| `native/gpui/src/settings.rs` | 旧配置导入、字段校验、原子保存与助手配置 |
| `native/gpui/src/helper.rs` | 输入进程生命周期和有界事件队列 |
| `native/gpui/src/ui.rs` | 应用状态、GPUI 轮盘、动画、设置与显示器管理 |
| `native/gpui/src/macos.rs`、`macos.m` | 透明窗口、全局坐标、菜单栏与 AVAudioPlayer 音效 |
| `native/gpui/bundle.py` | 素材裁剪、应用包、临时签名、ZIP 与体积报告 |

透明悬浮层使用 GPUI 的非激活面板，再通过 AppKit 配置鼠标穿透、窗口层级和 Space 行为。GPUI 0.2.2 的 macOS 显示器边界会把原点归零，因此此实现从 CoreGraphics 读取全局坐标，处理主屏左侧或上方的显示器。

输入事件通过有界队列进入主线程。窗口仅在状态变化或动画未结束时请求绘制。菜单栏事件定时检查，显示器布局每两秒核对一次。退出时结束输入助手并回收进程。

GPUI 创建窗口时会同步渲染首帧。窗口创建安排在模型更新结束后，避免首帧读取正在修改的模型。纯业务逻辑和输入协议测试留在独立 Rust 库中。

框架依据：[GPUI 官方介绍](https://gpui.rs/)、[GPUI 0.2.2 窗口选项](https://docs.rs/gpui/0.2.2/gpui/struct.WindowOptions.html)。构建与当前功能范围见 [macOS 原生版说明](../native/gpui/README.zh-CN.md)。

## 本次验证

原有 C++ 助手的 381 项检查与新增的 7 项 Rust 测试通过。`cargo fmt --all --check` 和严格 Clippy 检查通过。

在 macOS 图形会话中启动应用，完成设置页、轮盘与编辑区的截图检查。GUI 冒烟测试验证模拟输入、信号选择、音效调用、设置写入和滚动位置，并检查原生窗口的鼠标穿透、不激活、置顶与 Space 标志。

ZIP 解压后的 `.app` 通过签名校验，并从 `/tmp` 独立启动，完成同一套 GUI 冒烟测试。原生窗口标志检查与模拟输入测试不代表真实键鼠、外接显示器和全屏场景已全部验收。
