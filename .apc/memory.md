# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: 全面重构工作区几何计算机制，彻底移除外层圆角与写死缝隙，实现 100% 动态严丝合缝全屏 HUD 覆盖。
- **Latest progress**: 
  1. **直角沉浸 HUD**：彻底移除外层 `border-radius: 20px` 设为纯直角 `0px`，移除外边框，背景统一为沉浸暗色，四角与边缘彻底消灭透底现象；
  2. **EWMH _NET_WORKAREA 通用动态适配**：摒弃“假定任务栏只在左侧/顶部”与 `-16px/-12px` 的写死扣减，直接信赖窗口管理器权威发布的净工作区尺寸与坐标，自适应任意桌面环境（左/右/顶/底或隐藏任务栏）；
  3. **X11 协议级绝对几何锁定**：在 `ConfigureWindow` 中直接传入 `workarea.x/y/width/height`，物理级强行对齐；
  4. **三栏精密内边距对齐**：三栏宽度严格传入 `logical_width - 40`，加上容器左右 padding 恒等于 `logical_width`，`xwininfo` 验证物理右侧和底部缝隙均为 **0 像素**（Corners: `+132+64 -0+64 -0-0 +132-0`）；
  5. 门禁测试全绿，Clippy 0 警告，Release 构建并在后台持续运行中。
- **Next step**: 用户验证全新无圆角、零缝隙、全屏覆盖表现；随后推进切片 S3（快捷键录入交互组件）与 Milestone M2。
- **Known issues**: 无。通用多桌面全屏覆盖与 8ms 极速快捷键已全部就绪。

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
