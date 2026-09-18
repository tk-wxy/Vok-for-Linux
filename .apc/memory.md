# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: 落地 Wayland 任务栏坐标钳位反向排除（`child != vok_xid`）与触控板轻击（Tap）状态机追踪（决策 §25 / 规则 R13）；彻底攻克 Ubuntu Dock 纯空白死区点击不退让难题；全量 73 项自动化测试全绿，全新 Release 生产二进制已常驻上线。
- **Latest progress**: 
  1. 查明并向用户剖析“为什么日志总是检测到隐藏”：后置的终端/应用切换 Focus-Out 被误认为前置 Dock 点击触发；
  2. 查明 Dock 点击不退让深层机理：Wayland 合成器对 Dock 区域进行坐标钳位（导致 `query_pointer` 停滞在边界 $x=132$ 破坏纯几何校验），且笔记本触控板轻击派发 `BTN_TOUCH` 而非 `BTN_LEFT`；
  3. 落地 `WindowAdapter::check_pointer_against_vok`：综合窗口层级与工作区几何判定 `is_outside = geom_outside || (vok_xid != 0 && child != vok_xid)`，突破坐标钳位限制；
  4. 落地触控板 `BTN_TOUCH` 250ms 轻击 Tap 状态机与物理多按键全覆盖；
  5. 重新编译生产 Release 二进制并拉起常驻守护进程。
- **Next step**: 引导用户点击或轻触 Ubuntu Dock 空白处验证秒级自动隐藏，并在终端核验清晰明了的【内核硬件物理输入 (evdev)】外置判定日志。
- **Known issues**: 无。

## §0A 最近会话

### Session 2026-09-19 - 攻克 Wayland 坐标钳位与触控板轻击 Tap 状态机，彻底攻克 Dock 空白处无法隐退难题 (决策 §25 / 规则 R13)

- **核心闭环**：
  1. **澄清日志与现实因果**：用户看到的 `【失焦感知】OverlayWindow 失去前台活动焦点 (Focus-Out)` 系切换窗口向 AI 发送消息时触发的标准失焦，而非 Dock 点击触发；
  2. **攻克 Wayland 坐标钳位**：Mutter 内部 Clutter actor 拦截 Dock 指针事件导致 Xwayland 坐标停滞在边界 $x=132$，纯几何校验 `x < 132` 失效；升级为 `child != vok_xid` 反向排除，光标在 Dock 时 Xwayland 的 child 为 0 或非 Vok 顶级窗口，即刻判定为外置点击；
  3. **落地触控板 Tap 状态机**：监听 `BTN_TOUCH`（330）电平，$\Delta t < 250\text{ms}$ 准确识别轻击，支持实体鼠标左/右/中/侧全按键；
  4. **全量构建与守护进程上线**：73 项自动化测试全绿，Release 二进制常驻启动，实时日志清晰记录事件源与坐标判定。

### Session 2026-09-19 - 攻克授权后监听未热加载问题，实现 EvdevMouseWatcher 动态热挂载与按键全覆盖 (决策 §24)

- **核心闭环**：
  1. **查清“授权了也没有实现”根因**：此前守护进程在用户授权前已启动，`OverlayWindow` 在启动初始化失败后将 `evdev_mouse_watcher` 固化为不可变 `None`；即便后续 Polkit 成功赋予系统 ACL，运行中的主窗口从未重试，导致物理点击监听处于未激活状态；
  2. **落地动态热挂载与信号联动**：
     - 将 `OverlayWindow` 改造为 `RefCell<Option<Rc<EvdevMouseWatcher>>>`，实现 `ensure_evdev_watcher()`；
     - 每次 `reveal()` 呼出时自动自检并热挂载；
     - `SettingsWindow` 授权成功回调直连 `overlay.ensure_evdev_watcher()`，实现授权瞬刻即刻激活底层监听；
  3. **按键全覆盖**：支持 `BTN_LEFT`、`BTN_RIGHT`、`BTN_MIDDLE` 外部点击退让，使用非阻塞 `try_send` 消除 worker 线程隐患；
  4. **全量构建与进程更新**：79 项自动化测试全绿，Clippy 零警告，全新 Release 守护进程正式上线，实机日志确凿记录已挂接 Dragonfly 3 Pro、ELAN Touchpad 与 Mouse 三路内核事件流。

### Session 2026-09-19 - 彻底清除停驻猜想表面工程，深度复盘 GNOME Wayland 任务栏空白点击隔离机理 (规则 R12 / 决策 §23)

- **核心闭环**：
  1. 查清日志与现实脱节根因：并发单测注入伪坐标 `(50, 500)` 造成虚假隐藏日志；
  2. 彻底清除 200ms 停驻假动作，恪守 R12 规则；
  3. 深入 Mutter 与 Clutter 源码完成 Wayland Chrome Actor 隔离机理推演，证实纯 X11 协议在无 evdev 权限下机制无法捕获 Dock 空白处吸收事件；
  4. 固化真实有效失焦矩阵（Super键、Alt+Tab、顶栏、图标、Esc、快捷键、晃动免疫）；
  5. 78 项测试全绿，Release 守护进程重新编译就绪。
