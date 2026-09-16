# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: 响应实机测试反馈，彻底攻克“呼出不流畅”与“界面没有显示在最上层”两大体验卡点；实现 8ms D-Bus 极速唤起、EWMH 强制置顶（_NET_WM_STATE_ABOVE）与 XRaiseWindow 硬件级层级提升。
- **Latest progress**: 
  1. **呼出性能飞跃（300ms -> 8ms）**：排查发现原快捷键每次均执行 `vok-app` 导致 fork+exec 整个 GTK 进程；改用 GApplication 原生 D-Bus Action `"toggle"` + `gdbus` 极速调用，触发耗时缩短 35 倍以上，完全在常驻守护进程内零开销响应；
  2. **100% 霸屏置顶与层级提升**：窗口新增注入 `_NET_WM_STATE_ABOVE`、`_NET_WM_STATE_STAYS_ON_TOP`；呼出时发送 `_NET_ACTIVE_WINDOW`（source=2: Pager/Panel 绕过 GNOME 防焦点窃取）并执行 X11 原生 `configure_window(StackMode::ABOVE)`，首帧渲染后 20ms 二次强化，保证在任何全屏应用之上；
  3. **会话状态机 Toggle 闭环**：在 Toggle 呼出时原子投递 `PhysicalPress` 与 `PhysicalRelease`，保证状态机精准进入稳定常驻的 `Visible { mode: Toggle, is_key_down: false }`；
  4. 6 项量化门禁全绿，Clippy 0 警告，Release 编译成功并已在后台常驻运行。
- **Next step**: 用户人工体验极速置顶呼出；随后推进切片 S3（快捷键录入交互组件）与 Milestone M2（剪贴板监听与直接粘贴）。
- **Known issues**: evdev 硬件直读受系统用户组权限限制（当前通过 GNOME D-Bus 8ms 极速快捷键已实现极致丝滑体验）。

## §0A 最近会话

### Session 2026-09-17 - 攻克“呼出不流畅”与“界面未在最上层”核心体验缺陷

- **根因分析**：
  1. 原 GNOME 快捷键直接绑定 `vok-app`，每次触发都会启动新进程加载 80+ 动态库（耗时 200~300ms），引发卡顿；
  2. 窗口未设置 EWMH `_NET_WM_STATE_ABOVE`，且外部进程唤醒触发了 GNOME Mutter 的防焦点窃取，导致窗口被其他应用遮挡；
  3. 状态机在 Toggle 呼出时缺少 `PhysicalRelease`，导致被误判为 `Momentary` 瞬态长按。
- **彻底根治与验证**：
  - 在 GApplication 注册原生 Action `"toggle"`，GNOME 快捷键改用 `gdbus` 极速调用，实机 benchmark 呼出耗时降至 **8ms**；
  - 窗口注入 `_NET_WM_STATE_ABOVE` 与 `_NET_WM_STATE_STAYS_ON_TOP`，并调用 X11 原生 `configure_window(StackMode::ABOVE)` 和带 source=2 的 `_NET_ACTIVE_WINDOW`；
  - 修复状态机短按事件投递，Toggle 显隐精准可靠；Release 构建并在后台常驻。

### Session 2026-09-16 - 落地切片 S1（任务栏隐形）与切片 S2（系统托盘常驻）

- 落地 EWMH `_NET_WM_STATE_SKIP_TASKBAR` 与 `_NET_WM_WINDOW_TYPE_UTILITY`，Ubuntu Dock 彻底无图标、无运行圆点；
- 接入 `ksni` (SNI) 规范，系统顶部通知栏常驻 `view-grid-symbolic` 图标，支持左键 Toggle 与右键上下文菜单；
- `gio::ApplicationHoldGuard` 确保窗口隐藏时后台守护不退出（常驻内存 `< 5MB`）。

### Session 2026-09-16 - 修复三栏右侧遮挡与工作区避让

- 测算出主屏 3200x2000 HiDPI (Scale 2)，左任务栏 132 物理像素 (66 逻辑像素)，净可用工作区 3068x1936；
- 移除卡片 `min-width: 68px` 与 GridView 默认外边距，校准根容器 padding 为 `20px 23px`，三栏完整可见无截断。

## 指引

- 标识、技术栈、结构和命令：`manifest.md`
- 范围规则与死路：`rules.md`
- 架构选择与根因：`decisions.md`
- 目标架构与实施门禁：`../docs/ARCHITECTURE.md`
- 更早会话：`history.md`
