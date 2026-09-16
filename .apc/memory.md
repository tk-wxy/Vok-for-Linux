# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: 切片 S1（任务栏彻底隐形）与切片 S2（StatusNotifierItem 托盘常驻与静默守护）已 100% 落地并通过实机验证，彻底对齐 Windows 版 Vok 的“幽灵 HUD 覆盖层 + 零任务栏占用 + 纯净托盘常驻”体验。
- **Latest progress**: 
  1. **切片 S1（任务栏彻底隐形）**：`vok-infra::WindowAdapter` 成功接入 X11 / EWMH 属性注入，为窗口动态施加 `_NET_WM_STATE_SKIP_TASKBAR`、`_NET_WM_STATE_SKIP_PAGER` 与 `_NET_WM_WINDOW_TYPE_UTILITY`；安装带 `NoDisplay=true` 的 `com.vok.desktop` 避免应用列表污染；实机 `xprop` 校验 100% 具备 `SKIP_TASKBAR`，Ubuntu Dock（左侧任务栏）彻底杜绝图标占用与指示灯跳动；
  2. **切片 S2（系统托盘常驻与守护闭环）**：基于现代 Linux 规范接入 `StatusNotifierItem` (SNI)，顶部系统托盘常驻 `view-grid-symbolic` 图标；左键点击即时 Toggle 显隐，右键菜单支持呼出与优雅退出；通过 `async_channel` 桥接与 `gio::ApplicationHoldGuard` 保障主窗口隐藏时守护进程在后台以 `< 5MB` 静默运行；
  3. 全工作区 51 项测试全绿，6 项量化门禁全绿，Clippy 0 警告，Release 生产二进制构建就绪。
- **Next step**: 启动切片 S3（快捷键录入组件）与 Milestone M2（剪贴板监听、富文本/图片/文件货架与极速直接粘贴）。
- **Known issues**: 真实硬件按键监听需用户处于 `input` 组（当前已完备支持 GNOME F12 快捷键 + 内置 Mock 仿真）；Wayland 下后续按计划接入 `gtk4-layer-shell`。

## §0A 最近会话

### Session 2026-09-16 - 落地切片 S1（任务栏隐形）与切片 S2（系统托盘常驻）

- 针对用户反馈“无感呼出，没有占有任务栏；只保留托盘”的体验诉求，完成深度分析规划并全面落地：
- **切片 S1 任务栏彻底隐形**：
  - 在 `vok-infra::window` 中实现 `apply_skip_taskbar_by_title`，通过 `x11rb` 为全屏窗口注入 `_NET_WM_STATE_SKIP_TASKBAR`、`_NET_WM_STATE_SKIP_PAGER` 与 `_NET_WM_WINDOW_TYPE_UTILITY`；
  - 在 `vok-infra::desktop::shortcut` 中落地 `ensure_desktop_entry()`，写入带有 `NoDisplay=true` 的 `com.vok.desktop.desktop` 并更新桌面数据库；
  - 在 `vok-app::main` 中默认以 `GDK_BACKEND=x11` 运行，确保 Mutter 完全遵循 EWMH 规则，使 Ubuntu Dock 彻底过滤 Vok 窗口，完全不占用任务栏。
- **切片 S2 系统托盘常驻与生命周期**：
  - 在 `vok-infra::desktop::tray` 中基于 `ksni` 规范实现 `StatusNotifierItem` D-Bus 托盘服务；
  - 顶栏显示 `view-grid-symbolic` 托盘图标，提供左键单击 Toggle 呼出/隐藏，右键上下文菜单提供【🚀 呼出工作台】与【🚪 退出 Vok】；
  - 通过 `async_channel::unbounded::<TrayCommand>` 实现 D-Bus 线程到 GTK4 主循环的零拷贝并发桥接；
  - 引入 `gio::ApplicationHoldGuard` 确保全窗口隐藏时常驻守护不退出，内存维持在 `< 5MB`；
  - 实机 D-Bus `busctl call ... Activate` 测试与 `xprop` 校验 100% 通过。

- 响应人工测试反馈的两大核心缺陷：“1. 缺少快捷键呼出/关闭；2. 界面右侧没有显示全，屏幕左侧有任务栏”。
- **快捷键闭环**：落地 `DesktopShortcut::ensure_gnome_shortcut("F12")` 自动写入 GNOME dconf 快捷键；改造 `VokApp` 单实例感知，重复激活触发 Toggle；修复状态机 Toggle 事件处理；增加终端回车即时模拟。
- **三栏右侧遮挡根因排查与根治**：
  - 测算出用户主屏 3200x2000 HiDPI (Scale 2)，左任务栏 132 物理像素 (66 逻辑像素)，净可用工作区 3068x1936；
  - 深入排查发现 `three_pane` 实际尺寸 1577px 远超期望 1470px，根因是 GTK4 `GridView` 子项被 Adwaita 施加默认边距，卡片 `min-width: 68px` 导致启动台最小尺寸达 466px；
  - 在 CSS 中覆盖 `.vok-launcher-grid > child` 与 `.vok-stage-grid > child`（`padding: 2px; margin: 0; min-width: 0;`），移除卡片 `min-width: 68px`，设置 `ScrolledWindow::propagate_natural_width(false)`；
  - 校准 `.vok-root-container` padding 为 `20px 23px`（加上 1px 边框闭合为 24px），使根容器精准收敛为 1518 逻辑像素；
  - `xwininfo -name "Vok Overlay" -stats` 实机验证：`Width: 3068`, `Corners: +132+64 -0+64 -0-0 +132-0`，零像素溢出，右侧剪贴板栏完整可见。

### Session 2026-09-16 - 里程碑 M1（三栏布局、启动台与中转站货架 MVP）全链路落地与量化门禁闭环

- 推进并完成了切片 1.5（启动台货架）、切片 1.6（文件临时中转站货架）以及切片 1.7（全链路集成与量化门禁压测）。
- 启动台实现 4 列 `GtkGridView` 视口回收、单击启动兼自动关闭、右键上下文菜单（置顶/移除）、外部拖入与默认项自填充。
- 中转站实现 2 列 `GtkGridView` 视口回收、`GtkDropTarget` 批量文件拖入单事务落库、`GtkDragSource` 反向拖出至外部应用（`FileList`）、`ThumbnailWorker` 跨线程安全主线程回填、右键菜单（置顶/打开/目录/路径/移除）与栏头“一键清理未置顶”。
- 落地 `crates/vok-app/tests/m1_gate_test.rs` 全部 6 项量化门禁并 100% 通过。

### Session 2026-09-16 - M1 架构分析与纵向切片规划归档

- 深入分析 M1 里程碑（三栏骨架与双货架 MVP）的核心技术约束与实现难点。
- 确立 7 大纵向实施切片。
- 用户确认方案，正式归档 `docs/m1_planning.md`，维护 §0A 3 次上限约束。

## 指引

- 标识、技术栈、结构和命令：`manifest.md`
- 范围规则与死路：`rules.md`
- 架构选择与根因：`decisions.md`
- 目标架构与实施门禁：`../docs/ARCHITECTURE.md`
- 更早会话：`history.md`
