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
