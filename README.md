# lolPing Native

[![Native macOS](https://github.com/class-undefined/lolping-native/actions/workflows/native-mac.yml/badge.svg)](https://github.com/class-undefined/lolping-native/actions/workflows/native-mac.yml)

独立的 Rust + GPUI macOS 桌面信号工具，源自 [darrenprx/lolPing](https://github.com/darrenprx/lolPing)。当前为第一阶段预览版。

## 运行与打包

这是第一阶段的 Rust + GPUI 实现。桌面渲染、设置和应用状态由 Rust 管理。输入暂时复用已有的 C++ 助手，窗口和音频通过小型 Objective-C 模块调用 macOS 接口。原生应用无需 Node.js、Electron 或 WebView。

当前构建目标为 Apple 芯片 Mac。需要 Rust 1.94.1 或更新版本、Python 3 和 Xcode 命令行工具。GPUI 固定为 0.2.2，依赖版本记录在根目录的 `Cargo.lock` 中。Metal 着色器在运行时编译，无需安装单独的 Metal 编译工具。

在项目根目录运行：

```bash
sh native/hook-helper/build.sh
cargo run --locked -p lolping-native
```

首次打开后，在设置页点击“辅助功能权限”。授权后点击“重新连接”。开发时运行的终端或宿主程序也可能需要辅助功能权限。

打包命令：

```bash
python3 native/gpui/bundle.py
open "release/native/lolPing Native.app"
```

脚本生成独立的 `.app` 和 ZIP，并执行临时签名校验。

生成 DMG 安装包：

```bash
sh native/gpui/dmg.sh
```

默认先构建应用，再生成压缩 DMG。已有应用包时，可用 `sh native/gpui/dmg.sh --skip-build` 跳过构建。脚本可从任意目录运行，需要 macOS 自带的 `hdiutil`、`ditto` 和 Xcode 命令行工具。

产物位于 `release/native/lolPing-Native-<版本>-<架构>.dmg`。打开 DMG 后，将 `lolPing Native.app` 拖到 `Applications` 即可安装。脚本校验镜像成功后才替换已有 DMG。

所有打包产物位于 `release/native/`。这个版本尚未经过 Apple 公证，临时签名更新后可能需要重新授予辅助功能权限。

## 已实现的功能

| 功能 | 行为 |
| --- | --- |
| 信号轮盘 | 八方向选择，中心死区取消，Esc 或右键取消 |
| 信号 | 11 种原有信号、图标弹出、扩散环、全力进攻双图组合 |
| 音效 | 原有 WAV 音效、音量、静音和轮盘提示音 |
| 输入 | Option、Control、Shift、Command 或鼠标侧键触发，可选触发键加单击 |
| 设置 | 三栏紧凑面板、轮盘拖拽替换与排序、撤销、单击信号，以及大小、持续时间、音量和触发设置 |
| 桌面悬浮层 | 每个显示器一个透明窗口、鼠标穿透、跨 Space、全屏辅助窗口 |
| 菜单栏 | 打开设置、暂停或恢复、退出 |
| 快捷键 | `⌃⌥P` 暂停或恢复，`⌘,` 打开设置，`⌘Q` 退出 |
| 显示器变化 | 每两秒检测显示器布局，变化时重建悬浮层 |

按住触发键，按下鼠标左键并拖动，在目标方向松开。轮盘沿用旧版的 86 点中心死区。边缘处的轮盘可能超出屏幕，但仍可通过拖动方向选择信号。

设置窗口采用信号库、轮盘和参数三栏布局。设置中的轮盘与桌面轮盘共用绘制逻辑，方向和槽位一致。

- 从信号库拖到轮盘扇区可替换信号；信号已在轮盘上时，两处位置交换。
- 在轮盘内拖动信号可调整顺序，中间的信号依次让位。
- 拖到中心、轮盘外或按 `Esc` 可取消。点击“撤销”或按 `⌘Z` 可恢复上一次布局。
- 点击信号库中的信号可试听并选中，再点击扇区也能放置。拖到下方“单击信号”栏可修改单击信号。
- 修改后自动保存。“恢复默认”同时重置轮盘和单击信号，也支持撤销。

关闭设置窗口后，应用继续运行，可从菜单栏重新打开。

## 设置与旧版共存

原生版写入独立文件：

```text
~/Library/Application Support/lolPing-native/settings.json
```

首次运行时，如果原生配置不存在，会从旧版 `lolPing/settings.json` 读取支持的字段。原有配置不修改。自定义按键、旧版房间和表情字段不会导入，暂不支持的触发键恢复为 Option。

**使用真实输入时，请退出另一版本。** 两个程序同时拦截同一触发键会互相干扰。原生版用文件锁限制自身重复运行。

预览模式使用模拟助手和临时设置，不读取真实键鼠输入，也不写用户配置：

```bash
cargo run --locked -p lolping-native -- --preview
```

预览模式自动展示轮盘，设置页可试听信号。退出时使用菜单栏或 `⌘Q`。

## 验证

```bash
sh native/hook-helper/build.sh
native/hook-helper/build/helper_tests
cargo fmt --all --check
cargo test --locked -p lolping-native
cargo clippy --locked -p lolping-native --all-targets -- -D warnings
cargo run --locked -p lolping-native -- --smoke-test
```

Rust 测试覆盖八方向几何、编辑器投放边界、配置归一化、布局交换与排序、素材完整性，以及真实助手的模拟协议。

GUI 冒烟测试先通过助手模拟桌面轮盘操作，检查信号选择、窗口标志和音效。随后向预览窗口发送原生鼠标与键盘事件，验证拖拽替换、排序、交换、撤销、取消、单击信号、点击放置和设置保存。测试还会缩小窗口，检查底部控件能否通过滚动访问。测试使用临时配置，完成后自动退出，需要已登录的 macOS 图形会话。

辅助功能授权后的真实输入、不同缩放比例的外接显示器、全屏应用和 Space 切换，需要在相应硬件与桌面环境中验收。模拟协议测试不能替代这些检查。

## 后续迁移范围

此阶段尚未实现房间、自动更新、表情和自定义图片。设置目前提供中文界面与预设触发键，尚未加入登录启动、自定义任意按键和快捷键编辑。信号动画已用 GPUI 重写，与原 CSS 动画不保证逐帧一致。

下一阶段先迁移房间协议与加密、局域网和中继传输，再实现原生更新流程。房间迁移需要用旧版协议测试向量验证互通。更新功能需要独立的原生产物命名与版本检查，避免下载 Electron 安装包。表情素材应单独评估压缩或按需安装，防止包体积再次增加约 49 MiB。

Windows 原生适配和将 C++ 输入助手改写为 Rust，另行安排。Electron 完整版和网站演示见上游 [darrenprx/lolPing](https://github.com/darrenprx/lolPing)。本仓库使用 `native-mac.yml` 工作流构建和测试原生版。

## 项目结构

- `native/gpui/`：Rust 应用、GPUI 界面、macOS 适配和打包脚本。
- `native/hook-helper/`：C++ 输入助手及其测试。
- `assets/`：原生首版需要的 29 个信号素材。
- `docs/native-migration.md`：迁移分析、体积测量和验证记录。

本地原生首版的应用包约 10.41 MiB，ZIP 约 5.90 MiB。这个版本不包含表情素材。每次打包会输出实际大小。CI 完成后，可在对应 Actions 运行页面下载构建产物。

## 来源与许可

原项目由 Danrui Meng 开发，代码使用 MIT 许可证；本仓库保留了原始 [LICENSE](LICENSE)。上游提交与迁移范围记录在 [NOTICE.md](NOTICE.md)。

信号图标、音效和衍生应用图标归 Riot Games 所有，不包含在 MIT 代码许可证内。本项目与 Riot Games 没有隶属或赞助关系。
