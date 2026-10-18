[![CI](https://github.com/Dowanw0w/rectangle-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Dowanw0w/rectangle-rs/actions/workflows/ci.yml)
[![Release](https://github.com/Dowanw0w/rectangle-rs/actions/workflows/release.yml/badge.svg)](https://github.com/Dowanw0w/rectangle-rs/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

# rectangle-rs

用 Rust 写的跨平台窗口贴边工具：**Windows + macOS**。托盘常驻、纯配置文件管理、不弹任何窗口。

灵感来自 [Rectangle](https://rectangleapp.com/)（macOS）、[RectangleWin](https://github.com/ahmetb/RectangleWin)（Windows）：一个快捷键把当前窗口吸附到屏幕的指定区域。

```
┌──────────────┬──────────────┐
│  左上角 ⌃⌥U  │  右上角 ⌃⌥I  │
├──────────────┼──────────────┤
│  左下角 ⌃⌥J  │  右下角 ⌃⌥K  │
└──────────────┴──────────────┘
```

## 下载安装

到 [Releases](https://github.com/Dowanw0w/rectangle-rs/releases/latest) 下载对应平台产物：

| 产物 | 说明 |
|---|---|
| `rectangle-rs-windows-x86_64.zip` | 解压得到 `rectangle-rs.exe`，双击运行；首次会被 SmartScreen 拦一下（未签名构建），选"更多信息 → 仍要运行" |
| `rectangle-rs-macos-arm64.zip` | Apple Silicon（M 系列） |
| `rectangle-rs-macos-x86_64.zip` | Intel Mac |
| `SHA256SUMS.txt` | 校验和，`sha256sum -c SHA256SUMS.txt` 可校验 |

运行时**没有主窗口**，只在托盘（macOS 为菜单栏）出现一个图标：

- Windows：任务栏右下角，可能收在 `^` 里，建议拖到常显区域；
- macOS：菜单栏上的小方块图标。

macOS 首次运行（产物是 ad-hoc 签名、无公证）：

```bash
unzip rectangle-rs-macos-arm64.zip
xattr -dr com.apple.quarantine rectangle-rs.app   # 或右键 Open
open rectangle-rs.app
```

然后到 **系统设置 → 隐私与安全性 → 辅助功能** 勾选 rectangle-rs（未授权时程序只能读托盘、无法移动任何窗口）。授权一次后重编译/覆盖安装仍然有效。

## 功能

一次操作的对象是**当前聚焦的窗口**（窗口操作不抢焦点、不改 Z 序）。

**只有分屏、最大化和取消最大化会改变窗口尺寸；所有"移动"动作只改位置。**

### 移动（默认绑定，尺寸不变）

| 动作 | 说明 | 默认热键 |
|---|---|---|
| 移动到左上角 / 右上角 / 左下角 / 右下角 | 窗口保持原尺寸，角贴到工作区（去掉菜单栏、Dock、任务栏后）对应角 | `ctrl+alt+u` / `i` / `j` / `k` |
| 靠左 / 靠右 | 贴左边/右边，另一轴居中 | `ctrl+alt+←` / `→` |
| 靠上 / 靠下 | 贴上边/下边，另一轴居中 | `ctrl+alt+↑` / `↓` |
| 居中 | 保持尺寸居中到工作区 | `ctrl+alt+c` |
| 移动到下一个 / 上一个显示器 | 按显示器排列顺序循环，保持尺寸（目标屏装不下就贴边对齐） | `ctrl+alt+n` / `ctrl+alt+shift+n` |

窗口比工作区还大时会溢出屏幕而不是被缩小；需要居中/贴边的那个轴上改为对齐起始边，避免标题栏跑出屏幕。

### 窗口状态

| 动作 | 说明 | 默认热键 |
|---|---|---|
| 最大化 | 铺满工作区（可用"取消最大化"还原） | `ctrl+alt+f` |
| 取消最大化 | 还原到上次操作前的尺寸；原本就铺满屏幕的窗口会缩到 80% | `ctrl+alt+r` |
| 最小化 | 最小化窗口 | `ctrl+alt+m` |

### 分屏（会改变尺寸，默认不绑定热键）

需要分屏就自己绑一个热键，或从托盘菜单「分屏」子菜单里点：

| 动作 id | 说明 |
|---|---|
| `tile-top-left` / `tile-top-right` / `tile-bottom-left` / `tile-bottom-right` | 吸附到工作区四分之一 |
| `tile-left-half` / `tile-right-half` / `tile-top-half` / `tile-bottom-half` | 吸附到工作区一半 |

### 托盘菜单

```
移动到左上角      ⌃⌥U        ← 每个动作也可以直接点，菜单里带当前热键
移动到右上角      ⌃⌥I
靠左 / 靠右 / 靠上 / 靠下
居中
最大化 / 取消最大化 / 最小化
移动到下一个显示器 / 上一个显示器
分屏（会改变窗口尺寸） ▸   ← 子菜单：四分之一、半屏，默认无热键
────────────
✓ 开机自启动
编辑配置文件
重新加载配置
────────────
打开日志文件
────────────
退出
```

- **开机自启动**：勾选即可，不需要管理员权限（Windows 写 `HKCU\...\Run`，macOS 写 `~/Library/LaunchAgents`）。
- **编辑配置文件**：用系统默认编辑器打开 `config.toml`。
- **重新加载配置**：改完配置点一下即时生效，不用重启程序。
- **打开日志文件**：排障用。

左键点击图标会刷新托盘提示并输出一行状态日志。

## 配置

| 平台 | 位置 |
|---|---|
| Windows | `%APPDATA%\rectangle-rs\config.toml` |
| macOS | `~/Library/Application Support/rectangle-rs/config.toml` |

首次运行会自动生成带注释的完整配置文件（可用环境变量 `RECTANGLE_RS_CONFIG_DIR` 覆盖目录，方便做便携版）：

```toml
version = 1

# 窗口与工作区边缘的间距（像素/点），范围 0..=100，只影响位置不影响尺寸
margin = 0

[hotkeys]
# ---------- 窗口移动（不改尺寸）----------
top-left = "ctrl+alt+u"
top-right = "ctrl+alt+i"
left = "ctrl+alt+left"
right = "ctrl+alt+right"
center = ""                  # 空字符串 = 禁用该动作的热键
…
# ---------- 窗口状态 ----------
maximize = "ctrl+alt+f"
restore = "ctrl+alt+r"
minimize = "ctrl+alt+m"
# ---------- 多显示器 ----------
next-display = "ctrl+alt+n"
previous-display = "ctrl+alt+shift+n"
# ---------- 分屏（会改变尺寸，默认空 = 不绑定）----------
tile-left-half = "ctrl+alt+1"
tile-right-half = ""
```

规则：

- **热键语法**：`修饰键 + 主键`，用 `+`（或 `-`/空格）连接。
  - 修饰键：`ctrl` / `alt` / `shift` / `platform`；`platform` 在 macOS 是 ⌘、Windows 是 Win，别名 `cmd` / `command` / `super` / `win` / `meta`。
  - 主键：`a-z`、`0-9`、`up/down/left/right`、`space`、`enter`、`tab`、`home/end`、`pageup/pagedown`、`delete`、`f1`-`f24`，以及 `minus`/`equal`/`comma`/`period`/`slash`/`backslash`/`semicolon`/`quote`/`backquote`/`bracketleft`/`bracketright`。
  - 至少需要一个修饰键。
- **空字符串 = 禁用**（`center = ""`）；**整行不存在 = 使用内置默认热键**。所以想彻底关掉某个动作的热键，必须写 `""`，不能删行。
- 旧写法 `left-half` / `right-half` / `top-half` / `bottom-half` 仍然可用，含义等同于 `left` / `right` / `top` / `bottom`（只移动）。真正改尺寸的分屏动作是 `tile-*`。
- 同一份配置在 Windows 与 macOS 之间通用，`platform` 会自动映射成 ⌘ / Win。
- 非法/重复条目不会导致启动失败：会记日志并回退到默认值，托盘提示里也能看到。

## 构建与运行

需要 Rust stable（≥ 1.90）。

```bash
cargo run -p rectangle-rs          # 开发运行，直接进托盘
cargo build -p rectangle-rs --release
```

macOS 建议打一个 bundle（TCC 权限记忆更稳）：

```bash
cargo build -p rectangle-rs --release
./scripts/bundle-macos.sh
open target/rectangle-rs.app
```

### 测试

```bash
# 纯逻辑 crate：几何、平铺、热键解析、配置、动作引擎（任何平台都能跑）
cargo test -p rectangle-core

# 交叉类型检查平台后端（不需要目标平台机器）
cargo check -p rectangle-platform --target x86_64-pc-windows-gnu
cargo check -p rectangle-platform --target aarch64-apple-darwin

# GUI/托盘完整构建：只在 Windows / macOS 上可编译
cargo build -p rectangle-rs
```

## 排障

GUI 子系统程序没有控制台，所有信息都写到日志里：

| 平台 | 日志 |
|---|---|
| Windows | `%APPDATA%\rectangle-rs\rectangle-rs.log` |
| macOS | `~/Library/Application Support/rectangle-rs/rectangle-rs.log` |

托盘菜单里有 **"打开日志文件"**；崩溃时会弹窗给出 panic 信息、源文件位置和日志路径。

启动异常时可用环境变量做二分定位（PowerShell）：

```powershell
$env:RECTANGLE_RS_NO_TRAY = "1";    .\rectangle-rs.exe   # 跳过托盘图标
$env:RECTANGLE_RS_NO_HOTKEYS = "1"; .\rectangle-rs.exe   # 跳过全局热键注册
```

需要看控制台输出时，用带控制台的构建：

```bash
RUSTFLAGS="--cfg rectangle_rs_console" cargo build -p rectangle-rs --release
```

日志里还能看到 GPUI/DirectX 自身的报错（例如渲染器创建失败、字体回退）。

## 项目结构

```
crates/core        rectangle-core      纯逻辑：几何、平铺算法、热键解析、配置、动作引擎（含单元测试）
crates/platform    rectangle-platform  平台后端：Win32 / macOS Accessibility 实现同一个 WindowSystem trait
crates/app         rectangle-rs        托盘 daemon：托盘菜单、全局热键、开机自启、配置文件
```

设计要点、坐标系换算和平台坑位见 [docs/architecture.md](docs/architecture.md)。

## 已知限制

- 没有图形设置界面：改配置 = 编辑 `config.toml` + 托盘"重新加载配置"。
- 全屏（Space）或特殊窗口（部分 UWP 窗口、自绘游戏窗口）可能拒绝调整尺寸，日志里会写明原因。
- "取消最大化"的记忆只在本次运行内有效（重启后不保留）。
- Linux、Windows ARM64 尚未纳入发布矩阵。

## 许可证

MIT。见 [LICENSE](LICENSE)。
