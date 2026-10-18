# 架构与平台实现说明

本文记录 rectangle-rs 的关键设计决策、坐标系统换算，以及各平台的坑位，方便后续维护和扩展。

## 1. 分层

```
        ┌──────────────────────────────┐
        │ crates/app  (rectangle-rs)   │  托盘 daemon：托盘菜单、全局热键、开机自启、配置
        └──────────────┬───────────────┘
                       │ WindowSystem trait
        ┌──────────────┴───────────────┐
        │ crates/platform              │  Win32 / macOS Accessibility
        └──────────────┬───────────────┘
                       │ Rect / Display / Action / Engine
        ┌──────────────┴───────────────┐
        │ crates/core                  │  纯逻辑，无 OS 依赖，全部单测覆盖
        └──────────────────────────────┘
```

设计原则：**"窗口该去哪"全部在 core 里决定并被测试，"怎么把窗口挪过去"才交给平台层。**
平台层只实现 5 个方法（取当前窗口、读 frame、写 frame、最小化、枚举显示器）+ 两个可选钩子（记住焦点窗口、权限状态）。

因此：

- `cargo test -p rectangle-core` 在 Linux CI 上就能验证所有贴边行为；
- 平台层的代码量小到可以人工审阅，FFI 只是薄胶水；
- 新增平台（例如 Linux/X11）只需要实现 `WindowSystem`。

## 2. 坐标系统

所有 `Rect` 都以**主显示器左上角为原点、y 向下增长**，单位是"屏幕单位"：

- Windows：物理像素（进程声明 Per-Monitor DPI Aware v2，因此 Win32 不做坐标虚拟化）；
- macOS：point（视网膜屏上 1 point = 2 像素，与 AX API 一致）。

### Windows 细节

1. **DPI 感知**：`System::new()` 尽早调用 `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)`（失败即已声明，忽略）。
2. **可见边框偏移**：`GetWindowRect` 包含 Win10/11 的隐形缩放手柄（左右各约 7-8px，底部更多），直接按它贴边会让窗口"溢出"半个屏幕。
   因此：
   - 读窗口 frame 时用 `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)`（用户真正看到的边界），DWM 不可用时回退到 `GetWindowRect`；
   - 写窗口 frame 时，先同时取两套 rect 算出四边差值 (left/top/right/bottom)，再把目标视觉 rect 反向扩成 `SetWindowPos` 需要的窗口 rect。
3. **最大化的窗口**：`IsIconic` / `IsZoomed` 为真时先 `ShowWindow(SW_RESTORE)`，否则 `SetWindowPos` 会被系统忽略。
4. **枚举显示器**：`EnumDisplayMonitors` + `GetMonitorInfoW`（`MONITORINFOEXW` 取设备名），`rcWork` 即工作区；显示器 id 用 `HMONITOR` 指针值。
5. **不抢焦点**：`SetWindowPos` 带 `SWP_NOZORDER | SWP_NOACTIVATE`。

### macOS 细节

1. **辅助功能 API**：`AXUIElementCreateApplication(pid)` → `AXFocusedWindow` → 读写 `AXPosition` / `AXSize`（`AXValueCreate` + `CGPoint`/`CGSize`），最小化写 `AXMinimized`，全屏检测读 `AXFullScreen`（全屏时不操作，直接报错让用户先退出全屏）。
2. **写入顺序**：先写 position、再写 size、再写一次 position。不少应用（含 Finder / Xcode）会按旧屏幕边界夹紧第一次尺寸变更，第二次 position 修正这个漂移。
3. **坐标系换算**：AX 用"主屏左上角为原点、y 向下"；AppKit 的 `NSScreen.frame` 用"左下角为原点、y 向上"。
   换算公式（primary_top = 主屏 frame 的 maxY）：

   ```text
   ax_y = primary_top - (ns_y + ns_height)
   ```

   副屏坐标可能为负（在主屏左侧或上方），公式本身天然支持。
4. **主屏判定**：`NSScreen.screens[0]` 是带菜单栏的主屏（不是 `mainScreen`，后者跟随键盘焦点）。
5. **权限**：`AXIsProcessTrusted()` 检查；未授权时所有 AX 调用返回 `kAXErrorAPIDisabled`(-25211)，映射为 `PlatformError::PermissionDenied` 并在界面提示 + 提供跳转系统设置的按钮。
6. **对象生命周期**：`AXUIElementCreateApplication` / `AXUIElementCopyAttributeValue` 都返回 +1 的 CoreFoundation 对象，用 `CfOwned`（Drop → `CFRelease`）托管；当前目标窗口的 AX 元素缓存在 `System` 里，使其 `WindowId`（元素指针）在缓存期内保持有效。

## 3. 动作语义

| 动作 | 实现位置 | 语义 |
|---|---|---|
| 移动族：四角 / 四边 / 居中 | `tiling::move_frame` | 纯函数：保持 `current` 的尺寸，只改 x/y；`margin` 只影响位置；尺寸超过工作区时允许溢出，居中轴改为对齐起始边 |
| 分屏族：`tile-*` | `tiling::tile_frame` | 工作区的 1/4 或 1/2，统一 `inset(margin)` 后四舍五入 |
| 最大化 | `Engine::perform` | 工作区 inset(margin) |
| 取消最大化 | `Engine::restore` | 优先用"上一次操作前的 frame"（`RestoreCache`，按窗口存在内存里，取出即消费）；没有记录时，若窗口本就铺满工作区则缩到 80% 居中 |
| 最小化 | `WindowSystem::set_minimized` | Win32 `SW_MINIMIZE` / AX `AXMinimized = true` |
| 切换显示器 | `tiling::frame_on_target_display` | **保持尺寸**，按"相对显示器中心的比例偏移"映射位置，只夹位置不缩尺寸（装不下就对齐到目标工作区左上角） |

显示器顺序：按 `(x, y)` 升序排序（左→右、上→下），`neighbour_of` 环形取下一个/上一个，因此"循环"是天然的。

约束：**只有 `tile-*` / `maximize` / `restore` 允许改变窗口尺寸**，其余动作（移动、切换显示器）严格保持尺寸。`Action::move_anchor()` 与 `Action::tile_layout()` 把这条约束编码进类型，引擎按二者分支，测试里逐个动作断言宽高不变。

## 4. 事件与线程模型

程序**没有设置窗口**，是一个纯托盘 daemon（配置来自 `config.toml`）：

```
OS 线程（std::thread）                      主线程（GPUI 事件循环）
─────────────────────────                   ────────────────────────────
global-hotkey 事件 ─┐
tray-icon 点击      ├─ async_channel ──→  cx.spawn 循环 → daemon.borrow_mut().handle_event()
muda 菜单点击       │                        └─ Daemon::perform → Engine → WindowSystem
500ms 轮询 tick     ┘
```

- `Daemon` 放在 `Rc<RefCell<Daemon>>` 里，只在主线程访问；事件循环是 `cx.spawn(async { while let Ok(ev) = rx.recv().await { ... } })`。
- GPUI 仍然被链接，但**只用来提供平台事件循环和 foreground executor**（`global-hotkey` 需要主线程消息循环 / run loop，托盘/NSStatusItem 也需要），不创建任何窗口。要彻底去掉它需要自己实现 Win32 `GetMessage` 循环与 macOS `CFRunLoop`+`NSStatusItem` 宿主，收益是少几 MB 体积，留作后续可选优化。

- **热键管理器必须在主线程创建**：`global-hotkey` 在 Windows 上依赖创建线程的 Win32 消息循环（GPUI 的 Windows 后端就是标准 `GetMessageW`/`DispatchMessageW` 循环），在 macOS 上依赖主 run loop 投递 Carbon 事件（GPUI 的 `NSApplication` 提供）。
- 三个 OS 事件源各用一个**普通线程**阻塞读取，再通过 `async-channel` 送进 GPUI 的 foreground executor，避免在 UI 线程做阻塞等待。
- 所有窗口操作都在 GPUI 主线程执行：macOS 的 `NSScreen` 枚举要求主线程，且这样 AX 调用与 UI 状态天然串行。

### GPUI 借用规则（0.1.x 踩过的坑，保留作参考）

> 当前实现没有任何 GPUI 窗口，因此这一节不影响现有代码；保留是为了说明"为什么不做 GUI 窗口"，以及将来若要重新加 GUI 时别再踩。

GPUI 的 `Entity` 是一个 `RefCell`。**在 entity 的 `update` 闭包里，不能再对这个 entity 做任何会 lease 它的调用**，否则运行时 panic：

```
cannot update rectangle_rs::app::RectangleApp while it is already being updated
  @ gpui-0.2.2/src/app/entity_map.rs:116
```

会 lease entity 的调用包括（都踩过）：

- `App::open_window(...)` —— 它创建窗口后立刻 `window.draw()`，会重入根视图的 `render`；
- `WindowHandle::update(...)` —— 直接 lease 根视图（`show_window` 里"激活已有窗口"就是这种）。

因此 `crates/app/src/app.rs` 的约定是：

1. `RectangleApp::start()` 只做热键注册、显示器枚举、事件循环启动，**不开窗口**；
2. 打开窗口走关联函数 `RectangleApp::open_settings_window(entity, cx: &mut App)`，它从 `main.rs`（无借用）或 `App::defer` 回调里调用；
3. 从事件处理里请求窗口（托盘"打开设置"）时用 `cx.defer(move |cx| Self::bring_window_to_front(entity, cx))`，把操作推到当前 update 周期结束之后。

### 焦点策略（"操作哪个窗口"）

托盘菜单和"设置窗口在前台时按热键"都会让 rect-rs 自己成为前台应用，直接取"前台窗口"会拿到自己的窗口。因此：

1. `WindowSystem::focused_window()` 会比对进程 id，跳过 rect-rs 自己的窗口；
2. 平台层缓存"最近一个属于别的应用的窗口"（Windows 用 `HWND` + `IsWindow` 校验，macOS 持有一个 retain 的 AX 元素）；
3. 主线程每 500ms 调一次 `remember_foreground()` 保持缓存新鲜；关闭/最小化等失效情况会回退为 `NoFocusedWindow` 并给出提示。

## 5. 配置

- 位置：`%APPDATA%\rectangle-rs`、`~/Library/Application Support/rectangle-rs`（`RECTANGLE_RS_CONFIG_DIR` 可覆盖）。
- 解析策略：先读成宽松结构，再逐项校验；未知动作、非法热键、重复绑定都只产出**警告**并回退默认值，不阻断启动（警告写日志）。
- 语义：`hotkeys` 里写空字符串 = 显式解绑；不在配置文件里出现 = 使用默认热键。
- 写盘语义：`Config::to_toml` 会为**每一个动作**写一行，解绑的写成 `""`。若只写有绑定的动作，被清除的绑定会在下次启动时"复活"成默认值——因为"缺失"的语义是"用默认"（这是早期版本真实踩过的坑，已有测试锁住）。
- 首次启动会写一份带注释的模板（`crates/app/src/config_template.rs`，由 `Config::default()` 生成，不会和代码脱节）。
- 托盘"重新加载配置"＝重新读盘 + 全量重注册热键 + 刷新菜单里的热键提示，不重启进程。

## 6. 验证矩阵

| 检查项 | 在哪里验证 | 说明 |
|---|---|---|
| `cargo test -p rectangle-core`（84 个测试） | Linux / Windows / macOS | 几何、平铺、热键解析、配置、引擎全流程（含 mock WindowSystem） |
| `cargo clippy -- -D warnings` | 三平台 CI | 平台后端与 GUI 分别检查 |
| `cargo check -p rectangle-platform --target x86_64-pc-windows-gnu` | 任意主机（含 Linux） | 验证 Win32 FFI 类型与常量 |
| `cargo check -p rectangle-platform --target aarch64-apple-darwin` | 任意主机 | 验证 AX / objc2 用法 |
| `cargo build -p rectangle-rs`（Windows） | Windows runner | 链接 Win32 + 托盘 + 热键，产出 exe |
| `cargo build -p rectangle-rs`（macOS） | macOS runner | 同上，并额外产出 `.app` |
| 真机运行时行为 | 人工 | 贴边效果、权限流程、托盘菜单、热键录制 |

> GUI crate 依赖 GPUI，而 GPUI 的 HTTP 客户端会引入 `ring`（需要目标平台的 C 工具链），因此用 Linux 主机交叉 `check` GUI crate 不可行；GUI 部分由 Windows / macOS CI job 与真机验证覆盖。
>
> 另外 GPUI 的 **release** 构建依赖其 build script 在宿主上生成预编译着色器（Windows 走 `fxc`、macOS 走 Metal 工具链），所以 release 产物只能在 Windows / macOS 宿主上构建；Linux 主机上的 `cargo check/build --target ...-windows-gnu` 只适合 debug 验证。

## 7. 待办 / Roadmap

- 拖拽到屏幕边缘触发贴边（需要全局鼠标事件 + 区域判定）。
- 每个动作可配置"自定义尺寸/比例"（Rectangle 的 `-`/`+` 尺寸微调、1/3、2/3 分屏）。
- 多显示器下的"跨屏连续移动"（当前是循环跳转）。
- 记住窗口尺寸持久化到磁盘（当前只在内存）。
- Windows ARM64 与 Linux（X11/Wayland）后端。
- 可选的极简 GUI（例如只读展示当前配置 + 录制热键的 egui/原生对话框），保留纯 TOML 路径。
- macOS 若改用 `SMAppService` 需要签名 + bundle，当前用 LaunchAgent。
