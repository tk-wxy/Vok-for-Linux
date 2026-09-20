# 归档历史 (History)

> 仅用于归档超期会话（超出 `memory.md §0A` 三次上限的历史记录）与废弃决策/规则演进追溯。日常启动请勿读取本文件。

---

## 归档会话

### Session 2026-09-10 - 初始化项目规划与建立 APC 架构体系

- 用户确认废弃原基于 Tauri/Web 的 Workbench-APP 架构，启动原生 Linux 效率桌面工作台 Vok (Linux)。
- 明确核心交互为“中转站 (Stage) + 拖拽中全局呼出并丢入 (Mid-drag Summon) + 全屏沉浸面板”。
- 确立技术选型：Rust + GTK 4 + gtk4-layer-shell + Linux 内核 evdev / uinput + SQLite WAL。
- 完整搭建 APC 知识库与规则体系：`.apc/manifest.md`、`.apc/decisions.md`、`.apc/rules.md`、`.apc/workflow.md`、`.apc/memory.md`、`docs/ARCHITECTURE.md`、`docs/MILESTONES.md` 以及根目录各 Agent 适配器文件。

### Session 2026-09-12 - 总体功能规划与核心交互范式深度确立

- 用户全面补充总体功能规范：
  1. 界面与焦点：全屏覆盖避让任务栏；快捷键/托盘呼出，失焦/Esc/快捷键关闭；呼出聚焦搜索框，关闭精准返还原前台输入控件；
  2. 1:2:1 横向布局：左启动台（25%）、中中转站（50%）、右剪贴板（25%），顶部常驻联动搜索框；
  3. 两个副界面：独立设置小窗口（托盘/齿轮双入口）与全局增强搜索视图（架构预留）；
  4. 启动台：一行 4 项，卡片高度整除防截断算法，左键单击启动并隐藏，右键菜单，拖动排序，外部拖入添加；
  5. 中转站：临时文件流动货架，双向拖拽，左键单击直接粘贴，右键菜单，拖动排序，高度与启动台完全对齐；
  6. 剪贴板：文本/图片/文件捕获，单击直接粘贴，复制，删除；
  7. 性能与持久化：采用视口回收虚拟滚动（`GtkGridView`/`GtkListView`），保障海量数据下内存恒定 < 40MB、CPU 低开销，SQLite 单写者串行持久化；
  8. 定位原则：Workbench-APP 作为功能与前端成熟范本，作为技术架构失败反面教材，绝不移植其代码。
- 全量更新 `.apc/manifest.md`、`.apc/decisions.md`（追加 §7~§9）、`.apc/rules.md`（追加 R8~R11）、`docs/ARCHITECTURE.md` 与 `docs/MILESTONES.md`。

### Session 2026-09-12 - 启动 M0 宿主内核开发与核心状态机落地

- 批准 M0 里程碑详细实施规划（`m0_planning.md`），定义 6 大纵向切片。
- 实机检测并自动安装 Rust 1.98.1 稳定版工具链。
- 初始化 Cargo Workspace 多 crate 架构：`crates/vok-core`、`crates/vok-infra`、`crates/vok-app`。
- 实现 `vok-core`：`GenerationId` 严格代次、`HotkeyMode`（250ms 长短按判定）、`SessionStateMachine`（无 sleep、原子因果推进与旧代过滤）。
- 实现 `vok-infra`：基于 Linux 内核 `evdev` 的物理电平事件监听器与 `MockInputMonitor` 仿真通道；实现 `WindowAdapter` 基于 X11 EWMH `_NET_WORKAREA` 的任务栏避让算法。
- 实现 `vok-app`：构建 GTK 4 全屏深色毛玻璃遮罩窗口、`GtkDropTarget` 外部拖拽入站中转、Esc 与空白处点击关闭，并通过 `async-channel` + `glib::spawn_future_local` 达成零 sleep 异步事件桥接。
- 编写并全绿通过 12 项单元测试，实机成功运行并准确计算当前多屏工作区避让（`3068x1936+132+64`），Release 生产二进制仅 1.5MB。

### Session 2026-09-16 - 修复三栏右侧遮挡与工作区避让

- 测算出主屏 3200x2000 HiDPI (Scale 2)，左任务栏 132 物理像素 (66 逻辑像素)，净可用工作区 3068x1936；
- 移除卡片 `min-width: 68px` 与 GridView 默认外边距，校准根容器 padding 为 `20px 23px`，三栏完整可见无截断。

### Session 2026-09-16 - 落地切片 S1（任务栏隐形）与切片 S2（系统托盘常驻）

- 落地 EWMH `_NET_WM_STATE_SKIP_TASKBAR` 与 `_NET_WM_WINDOW_TYPE_UTILITY`，Ubuntu Dock 彻底无图标、无运行圆点；
- 接入 `ksni` (SNI) 规范，系统顶部通知栏常驻 `view-grid-symbolic` 图标，支持左键 Toggle 与右键上下文菜单；
- `gio::ApplicationHoldGuard` 确保窗口隐藏时后台守护不退出（常驻内存 `< 5MB`）。

### Session 2026-09-17 - 落地切片 S3：可视化快捷键自定义与录入交互组件 (HotkeyRecorder)

- 彻底解耦硬编码 `F12` 快捷键，落地 `vok-core` 纯领域模型，支持 `Super + Space`、`Alt + Space`、`Ctrl + ~` 等任意标准组合；
- SQLite WAL 新增 `app_settings` 表，单写者串行持久化用户专属配置；
- GNOME `custom-keybinding` 动态热更新，修改即刻生效无需重启进程；
- 封装 `HotkeyRecorder` 交互组件与 `SettingsPopover` 浮层面板，主窗口内 Toggle 按键比对与底部 Footer 状态文本实时联动同步。

### Session 2026-09-17 - 根除窗口抽搐缩放与全屏未覆盖 bug，确立 Zero-Twitch 规范

- 定位 `UTILITY` 窗口被 Mutter 禁止最大化并居中放置的硬编码陷阱，恢复为标准 `NORMAL` 类型；
- 启用 GTK 原生 `window.maximize()` 首帧贴合，彻底消除居中留白与短暂未覆盖 bug；
- `set_skip_taskbar` 移除多余坐标操作，仅保留纯净 `StackMode::ABOVE`；
- 弱引用可见性防抖守卫与 GSettings 幂等性过滤，实测 `Corners: +132+64 -0+64 -0-0 +132-0` 零缝隙贴合。

### Session 2026-09-17 - 根治“界面抽搐抖动”，确立 Zero-Jitter 无抖动纯净映射规范

- **核心闭环**：
  1. 深入 GNOME Shell `windowManager.js` 源码，查明 `enable-animations=false` 反而导致 0.01 尺度首帧被渲染并暴力弹爆全屏的系统级缺陷；
  2. 查明 `OverlayWindow::reveal()` 中 20ms 定时器重复广播 `max_horz/max_vert` 导致 Mutter 触发 `MOVE_RESIZE` 二次重排的抖动根因；
  3. 彻底精简呼出链路，消除 20ms 定时器与冗余 X11 广播，呼出耗时降至 **4.7ms**，实测界面完全平稳、零抽搐。

### Session 2026-09-17 - 消除 Mutter 动效抖动、尺寸贴合与失焦关闭体验闭环

- 彻底查明 Mutter 窗口动效机理，确立无抖动纯净映射规范（Zero-Jitter Pure Mapping Policy）；
- 移除 0.01 强制尺寸重排 hacks，消除 20ms 闪烁；
- 建立 EWMH 双向焦点暂存与精准恢复引擎；
- 呼出时延降至 4.7ms，全量单元测试与门禁验证通过。

### Session 2026-09-17 - 落地 M2 切片 2.1：前台焦点暂存与原子精准返还引擎、失焦自动退出 (Focus-Out)

- **核心闭环**：
  1. 确立并落盘 `docs/m2_planning.md` 整体技术方案；
  2. `vok-infra::window` 封装 X11/EWMH `_NET_ACTIVE_WINDOW` 查询与恢复接口，单测覆盖非法 XID 容错；
    3. `OverlayWindow` 呼出时智能识别排除自身并暂存前台 XID，`hide()` 时原子触发恢复请求；
    4. 挂载激活状态通知，设计 `has_been_active` 状态守卫排除映射首帧伪失焦，实机 Toggle 与 xprop 验证焦点 100% 精准归还原应用。

### Session 2026-09-18 - 落地 M2 切片 2.2 硬件直接粘贴与切片 2.3 反向拖出透明代理锚点

- **核心闭环**：
  1. `vok-infra::input` 封装 `InputInjector`，建立 `/dev/uinput` 与 X11 XTEST 双通道，提供无感硬件击键；
  2. `vok-app` 联动实现单击/回车直接粘贴流水线，恪守 R7 延时时序守卫（50ms）；
  3. 查明 Wayland 拖出 Unmap 掐断与失焦时序冲突根因，落地 `vok-proxy-anchor` 全透明代理模式，保持 Surface 存活且视觉完全穿透；
  4. `StageView` 卡片挂载 `DragSource` 生命周期与多格式复合载荷；
  5. 门禁测试扩展至 GATE-M2-1~5，67 项自动化测试全部通过。

### Session 2026-09-18 - 落地 M3 前置 4 项核心功能：独立设置窗口、深浅色系统跟随、Esc捕获与多轨失焦隐藏

- **核心闭环**：
  1. 新建 `crates/vok-app/src/ui/settings_window.rs`，包含快捷键管理、深浅色切换、隐退行为设置与关于四大模块；
  2. `vok-infra::desktop::tray` 扩展 `VokTray` 接入设置回调，主界面 Header 齿轮点击原子关闭 Overlay 并呈现设置窗口；
  3. 新建 `crates/vok-app/src/ui/theme.rs` 与 `vok-core::theme`，对接 `adw::StyleManager` 与 `.theme-light` 样式集；
  4. 主窗口切换为 `PropagationPhase::Capture` 彻底解决 Esc 被 `SearchEntry` 拦截问题；
  5. 建立工作区 EWMH 自适应与 Super 键 D-Bus 概览监听。

### Session 2026-09-18 - 彻底根除晃动鼠标误退出，落地 XI2 RawButtonPress 全屏物理点击与任务栏沉淀感知闭环 (决策 §23)

- **核心闭环**：
  1. 定位并铲除表面工程病根：彻底移除 `pointer_leave_time` 与 `EventControllerMotion`，消除把光标移动误当成失焦点击的错误猜想；
  2. 新增 `OutsideClickWatcher`：通过 `xinput_xi_select_events` 监听 `XI_RawButtonPress`，被动捕获全局物理点击而不破坏指针状态；
  3. 维系 R3 规则守护：反向拖出代理模式（`is_in_proxy_mode`）自动挂起外部点击检测；
  4. 自动化集成测试全绿，Release 守护进程常驻就绪。

### Session 2026-09-19 - 彻底清除停驻猜想表面工程，深度复盘 GNOME Wayland 任务栏空白点击隔离机理 (规则 R12 / 决策 §23)

- **核心闭环**：
  1. 查清日志与现实脱节根因：并发单测注入伪坐标 `(50, 500)` 造成虚假隐藏日志；
  2. 彻底清除 200ms 停驻假动作，恪守 R12 规则；
  3. 深入 Mutter 与 Clutter 源码完成 Wayland Chrome Actor 隔离机理推演，证实纯 X11 协议在无 evdev 权限下机制无法捕获 Dock 空白处吸收事件；
  4. 固化真实有效失焦矩阵（Super键、Alt+Tab、顶栏、图标、Esc、快捷键、晃动免疫）；
  5. 78 项测试全绿，Release 守护进程重新编译就绪。

### Session 2026-09-19 - 攻克授权后监听未热加载问题，实现 EvdevMouseWatcher 动态热挂载与按键全覆盖 (决策 §24)

- **核心闭环**：
  1. **查清“授权了也没有实现”根因**：此前守护进程在用户授权前已启动，`OverlayWindow` 在启动初始化失败后将 `evdev_mouse_watcher` 固化为不可变 `None`；即便后续 Polkit 成功赋予系统 ACL，运行中的主窗口从未重试，导致物理点击监听处于未激活状态；
  2. **落地动态热挂载与信号联动**：
     - 将 `OverlayWindow` 改造为 `RefCell<Option<Rc<EvdevMouseWatcher>>>`，实现 `ensure_evdev_watcher()`；
     - 每次 `reveal()` 呼出时自动自检并热挂载；
     - `SettingsWindow` 授权成功回调直连 `overlay.ensure_evdev_watcher()`，实现授权瞬刻即刻激活底层监听；
  3. **按键全覆盖**：支持 `BTN_LEFT`、`BTN_RIGHT`、`BTN_MIDDLE` 外部点击退让，使用非阻塞 `try_send` 消除 worker 线程隐患；
  4. **全量构建与进程更新**：79 项自动化测试全绿，Clippy 零警告，全新 Release 守护进程正式上线，实机日志确凿记录已挂接 Dragonfly 3 Pro、ELAN Touchpad 与 Mouse 三路内核事件流。

### Session 2026-09-19 - 攻克 Wayland 坐标钳位与触控板轻击 Tap 状态机，彻底攻克 Dock 空白处无法隐退难题 (决策 §25 / 规则 R13)

- **核心闭环**：
  1. **澄清日志与现实因果**：用户看到的 `【失焦感知】OverlayWindow 失去前台活动焦点 (Focus-Out)` 系切换窗口向 AI 发送消息时触发的标准失焦，而非 Dock 点击触发；
  2. **攻克 Wayland 坐标钳位**：Mutter 内部 Clutter actor 拦截 Dock 指针事件导致 Xwayland 坐标停滞在边界 $x=132$，纯几何校验 `x < 132` 失效；升级为 `child != vok_xid` 反向排除，光标在 Dock 时 Xwayland 的 child 为 0 或非 Vok 顶级窗口，即刻判定为外置点击；
  3. **落地触控板 Tap 状态机**：监听 `BTN_TOUCH`（330）电平，$\Delta t < 250\text{ms}$ 准确识别轻击，支持实体鼠标左/右/中/侧全按键；
  4. **全量构建与守护进程上线**：73 项自动化测试全绿，Release 二进制常驻启动，实时日志清晰记录事件源与坐标判定。

### Session 2026-09-19 - 攻克 Dock 首次点击成功后全失效与图标偶发驻留，落地边界钳位容差与首帧预写入 (决策 §26 / 规则 R13)

- **核心闭环**：
  1. **查清 Dock 首次有效后全失效因果链路**：初次从 Dock 外呼出时 `child=0x0` 能退让，但一旦光标移入 Vok 再移入 Dock，Mutter 停止派发 motion 使 Xwayland 坐标停滞在 $x=133$ 且 `child=vok_xid` 永不重置，后续点击判定全为假；
  2. **落地 `border_clamped` 边缘容差算法**：利用 Vok 内部拥有 40px 绝对空白 padding 的几何事实，对避让边缘 6 物理像素内的点击权威判定为外置点击，彻底攻克连续点击全失效问题；
  3. **查清任务栏偶发驻留因果链路**：GTK 4 窗口映射到 ClientMessage 到达之间存在数毫秒属性空窗期，Mutter / GNOME Shell `ShellWindowTracker` 偶发抢先捕获并将 Vok 写入 Dock 运行栈；
  4. **落地 `window.realize()` 与 `change_property32` 预写入**：窗口在 Map 前即由 `change_property32(PropMode::APPEND)` 预置 `_NET_WM_STATE_SKIP_TASKBAR` 与 `_NET_WM_STATE_SKIP_PAGER`，消除首帧中间态；桌面项追加 `StartupNotify=false`；
  5. **全量构建与进程上线**：82 项自动化测试全绿，全新 Release 生产守护进程常驻上线。
### Session 2026-09-20 - 攻克长按无限震荡、输入法乱切与 350ms 迟滞，实现 0ms 极致跟手与输入法完全隔离 (决策 §27 ~ §30)

- **核心闭环**：
  1. **查清长按无限开闭交替震荡根因**：
     - Linux / GNOME 键盘长按重复延迟（Typematic Delay）为 500ms，连发频率为 30ms；旧逻辑在 85ms 内未等到第 2 发便误判为单击关闭窗口，而在 500ms 连发到达时又重新打开窗口；且 GNOME 子进程调度抖动（高达 286ms）穿透了 110ms 静默超时，形成无限震荡；
     - 升级为 350ms ~ 850ms 连发首发捕获窗口与 350ms 容差静默心跳，彻底终结了长按自动关闭与反复震荡；
  2. **查清输入法在“拼音”与“en”之间来回切换根因**：
     - UI 层 `key_controller` 仅拦截了 `key_pressed` 组合键，未监听 `key_released`，且放行了 `Alt_L` / `Alt_R` 单独按压；导致孤立修饰键释放事件泄露至 `search_entry` 和 Fcitx5，被输入法识别为中英切换指令；
     - 在 `Capture` 阶段全面拦截并消费热键涉及的修饰键（Alt/Ctrl/Shift/Super）按下与释放，彻底阻断其进入底层 `GtkIMContext`，实现输入法零干扰；
  3. **攻克长按松手关闭迟滞不跟手（350ms 延迟）难题**：
     - 在 `OverlayWindow` 中引入 `is_initial_press_held` 状态机与 `connect_key_released` 监听；
     - 窗口获取焦点后直接以微秒级（0ms）捕获物理松手事件，若按压时长 $> 280\text{ms}$（长按），立即瞬时隐藏窗口，真正实现“手抬即隐”、极致跟手；若 $\le 280\text{ms}$（短按），保持常驻展示并解锁搜索框文本输入；
  4. **全量验证与构建上线**：81 项测试全绿，Release 二进制已更新拉起实测，用户确认“测试符合预期”。
