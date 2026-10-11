# lolPing Native

[![Native builds](https://github.com/class-undefined/lolping-native/actions/workflows/native.yml/badge.svg)](https://github.com/class-undefined/lolping-native/actions/workflows/native.yml)

独立的 Rust + GPUI Windows / macOS 桌面信号工具，源自 [darrenprx/lolPing](https://github.com/darrenprx/lolPing)。当前为第一阶段预览版。

## 运行与打包

这是第一阶段的 Rust + GPUI 实现。桌面渲染、设置和应用状态由 Rust 管理。输入复用 C++ 助手。macOS 通过 Objective-C 调用 AppKit 和 AVFoundation；Windows 通过 C++ 调用 Win32 和 WinMM。原生应用无需 Node.js、Electron 或 WebView。

构建目标为 Windows x64 和 Apple 芯片 Mac。需要 Rust 1.94.1、Python 3.9 或更新版本。macOS 还需要 Xcode 命令行工具。GPUI 固定为 0.2.2，依赖版本记录在根目录的 `Cargo.lock` 中。Metal 着色器在运行时编译，无需安装单独的 Metal 编译工具。

### macOS

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

### Windows

支持 Windows 10 1809 及更新版本，需要支持 DirectX 11 的图形环境。安装 Visual Studio 2022 Build Tools，勾选“使用 C++ 的桌面开发”和 Windows SDK。打包还需要 [Inno Setup 6](https://jrsoftware.org/isinfo.php)。

在 x64 Visual Studio Developer PowerShell 中，切换到项目根目录运行：

```powershell
./native/hook-helper/build.ps1
cargo run --locked -p lolping-native
```

GPUI 的 Windows 构建需要 Windows SDK 中的 `fxc.exe`。如果它不在 PATH 中，设置：

```powershell
$fxc = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\fxc.exe" |
  Sort-Object FullName -Descending | Select-Object -First 1
$env:GPUI_FXC_PATH = $fxc.FullName
```

构建依据见 [GPUI 0.2.2 构建脚本](https://docs.rs/crate/gpui/0.2.2/source/build.rs)。

```powershell
python native/gpui/bundle_windows.py
```

脚本生成 `release/native/lolPing-Native-<版本>-windows-x64-setup.exe` 和便携 ZIP。安装包按当前用户安装，无需管理员权限。便携版需完整解压，再运行 `lolping-native.exe`，保留同目录的输入助手和 `assets` 文件夹。

Windows 默认用 Alt 触发轮盘，支持 Ctrl、Shift、Windows 键和鼠标侧键。系统托盘提供打开设置、暂停或恢复、退出。快捷键为 `Ctrl+Alt+P`、`Ctrl+,`、`Ctrl+Q` 和 `Ctrl+Z`。关闭设置窗口后，应用继续在托盘运行。

Windows 安装包尚未签名。真实键鼠、混合缩放的多显示器和游戏内覆盖效果仍需实机验收。独占全屏和以管理员身份运行的目标应用可能限制悬浮层或输入处理。

### GitHub 自动构建

每次推送分支或标签、提交 Pull Request，都会运行 [Native builds](https://github.com/class-undefined/lolping-native/actions/workflows/native.yml)。也可以在 Actions 页面手动运行。

| Actions 产物 | 文件 |
| --- | --- |
| `lolPing-native-windows-x64` | Windows 安装 EXE、便携 ZIP |
| `lolPing-native-macos-arm64` | macOS DMG、应用 ZIP |

打开对应运行记录，在 Artifacts 中下载。产物保留 14 天；此流程不自动创建 GitHub Release。Windows 任务还会静默安装 EXE，检查中文安装路径中的程序、输入助手和全部素材。

## 已实现的功能

| 功能 | 行为 |
| --- | --- |
| 信号轮盘 | 八方向选择，中心死区取消，Esc 或右键取消 |
| 信号 | 11 种原有信号、图标弹出、扩散环、全力进攻双图组合 |
| 音效 | 原有 WAV 音效、音量、静音和轮盘提示音 |
| 输入 | 平台对应的修饰键或鼠标侧键触发，可选触发键加单击 |
| 设置 | 三栏紧凑面板、轮盘拖拽替换与排序、撤销、单击信号，以及大小、持续时间、音量和触发设置 |
| 桌面悬浮层 | 每个显示器一个透明窗口、鼠标穿透；Windows 处理每屏缩放，macOS 支持跨 Space 和全屏辅助窗口 |
| 菜单栏 / 系统托盘 | 打开设置、暂停或恢复、退出 |
| macOS 快捷键 | `⌃⌥P` 暂停或恢复，`⌘,` 打开设置，`⌘Q` 退出 |
| 显示器变化 | 每两秒检测显示器布局，变化时重建悬浮层 |

按住触发键，按下鼠标左键并拖动，在目标方向松开。轮盘沿用旧版的 86 点中心死区。边缘处的轮盘可能超出屏幕，但仍可通过拖动方向选择信号。

设置窗口采用信号库、轮盘和参数三栏布局。设置中的轮盘与桌面轮盘共用绘制逻辑，方向和槽位一致。

- 从信号库拖到轮盘扇区可替换信号；信号已在轮盘上时，两处位置交换。
- 在轮盘内拖动信号可调整顺序，中间的信号依次让位。
- 拖到中心、轮盘外或按 `Esc` 可取消。点击“撤销”或按 `⌘Z`（Windows 为 `Ctrl+Z`） 可恢复上一次布局。
- 点击信号库中的信号可试听并选中，再点击扇区也能放置。拖到下方“单击信号”栏可修改单击信号。
- 修改后自动保存。“恢复默认”同时重置轮盘和单击信号，也支持撤销。

关闭设置窗口后，应用继续运行，可从菜单栏或系统托盘重新打开。

## 设置与旧版共存

原生版写入独立文件：

```text
macOS:   ~/Library/Application Support/lolPing-native/settings.json
Windows: %APPDATA%\lolPing-native\settings.json
```

首次运行时，如果原生配置不存在，会从旧版 `lolPing/settings.json` 读取支持的字段。原有配置不修改。自定义按键、旧版房间和表情字段不会导入，暂不支持的触发键恢复为 Alt（macOS 为 Option）。

**使用真实输入时，请退出另一版本。** 两个程序同时拦截同一触发键会互相干扰。原生版用文件锁限制自身重复运行。

预览模式使用模拟助手和临时设置，不读取真实键鼠输入，也不写用户配置：

```bash
cargo run --locked -p lolping-native -- --preview
```

预览模式自动展示轮盘，设置页可试听信号。退出时使用菜单栏、系统托盘或设置页的“退出”。

## 验证

```bash
sh native/hook-helper/build.sh
native/hook-helper/build/helper_tests
cargo fmt --all --check
cargo test --locked -p lolping-native
cargo clippy --locked -p lolping-native --all-targets -- -D warnings
cargo run --locked -p lolping-native -- --smoke-test
```

Windows 验证命令：

```powershell
./native/hook-helper/build.ps1
./native/hook-helper/build/helper_tests.exe
cargo fmt --all --check
cargo test --locked -p lolping-native
cargo clippy --locked -p lolping-native --all-targets -- -D warnings
cargo run --locked -p lolping-native -- --smoke-test
```

Rust 测试覆盖八方向几何、编辑器投放边界、配置归一化、布局交换与排序、素材完整性，以及真实助手的模拟协议。

GUI 冒烟测试先通过助手模拟桌面轮盘操作，检查信号选择、窗口标志和音效。随后向预览窗口发送原生鼠标与键盘事件，验证拖拽替换、排序、交换、撤销、取消、单击信号、点击放置和设置保存。测试还会缩小窗口，检查底部控件能否通过滚动访问。测试使用临时配置，完成后自动退出，需要已登录的图形会话和可用音频设备。macOS 编辑器测试发送原生窗口事件，Windows 编辑器测试向 GPUI 分发输入；Windows 的真实系统输入需单独验收。GitHub Actions 只运行非 GUI 测试与打包检查。

辅助功能授权后的真实输入、不同缩放比例的外接显示器、全屏应用和 Space 切换，需要在相应硬件与桌面环境中验收。模拟协议测试不能替代这些检查。

## 后续迁移范围

此阶段尚未实现房间、自动更新、表情和自定义图片。设置目前提供中文界面与预设触发键，尚未加入登录启动、自定义任意按键和快捷键编辑。信号动画已用 GPUI 重写，与原 CSS 动画不保证逐帧一致。

下一阶段先迁移房间协议与加密、局域网和中继传输，再实现原生更新流程。房间迁移需要用旧版协议测试向量验证互通。更新功能需要独立的原生产物命名与版本检查，避免下载 Electron 安装包。表情素材应单独评估压缩或按需安装，防止包体积再次增加约 49 MiB。

将 C++ 输入助手改写为 Rust，另行安排。Electron 完整版和网站演示见上游 [darrenprx/lolPing](https://github.com/darrenprx/lolPing)。本仓库使用 `native.yml` 工作流构建和测试原生版。

## 项目结构

- `native/gpui/`：Rust 应用、GPUI 界面、Windows / macOS 适配和打包脚本。
- `native/hook-helper/`：C++ 输入助手及其测试。
- `assets/`：原生首版需要的 29 个信号素材。
- `docs/native-migration.md`：迁移分析、体积测量和验证记录。

本地原生首版的应用包约 10.41 MiB，ZIP 约 5.90 MiB。这个版本不包含表情素材。每次打包会输出实际大小。CI 完成后，可在对应 Actions 运行页面下载构建产物。

## 来源与许可

原项目由 Danrui Meng 开发，代码使用 MIT 许可证；本仓库保留了原始 [LICENSE](LICENSE)。上游提交与迁移范围记录在 [NOTICE.md](NOTICE.md)。

信号图标、音效和衍生应用图标归 Riot Games 所有，不包含在 MIT 代码许可证内。本项目与 Riot Games 没有隶属或赞助关系。
