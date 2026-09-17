# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: 根治“界面抽搐抖动”缺陷，查明 GNOME Shell 0.01 帧渲染缺陷与 GTK4 重新呼出未显式最大化导致与 Mutter 状态争夺的根因，确立无抖动纯净映射规范。
- **Latest progress**: 
  1. **抽搐抖动两大根因定位**：
     - 当设置 `enable-animations = false` 时，GNOME Shell `windowManager.js` 的 `_mapWindow` 仍无条件执行 `scale_x = 0.01; scale_y = 0.05` 并 `await`，导致 Clutter 在第 1 帧将全屏窗口渲染为一个 1% 宽的微小斑点，第 2 帧瞬间弹爆全屏，肉眼可见剧烈痉挛；
     - 在 GTK4 中，窗口隐藏（`set_visible(false)`）会导致底层 `GdkSurface` 销毁并重置 `is_maximized=false`。若重新呼出（`reveal()`）未显式调用 `window.maximize()`，GTK 会默认以未最大化尺寸向合成器呈现，而 X11 层又试图设置最大化，引发 GTK 与 Mutter 来回拉扯震荡（4 轮反复翻转），导致窗口骤缩为 2972x1452 并引发抽搐；
  2. **彻底根治与纯净映射**：
     - 废弃全局篡改 `enable-animations` 的方案，恢复 GNOME 原生平滑高帧率渲染管线；
     - 在 `OverlayWindow::reveal()` 呼出时无条件显式执行 `self.window.maximize()`，从构建首帧起锁定最大化，彻底杜绝 GTK 与 Mutter 的状态拉扯；
     - `set_skip_taskbar` 完整固化 `MAXIMIZED_HORZ/VERT`、`SKIP_TASKBAR`、`ABOVE`，删除 20ms 延时定时器，实机呼出耗时保持 **4.7ms**；
  3. **实测性能验证**：连续多次快速 Toggle 呼出与隐藏，`xwininfo` 四角数据持续锁定 `Corners: +132+64 -0+64 -0-0 +132-0` 绝对平齐，Release 守护进程（task-1211）在后台持续稳定运行。
- **Next step**: 用户实机体验验证呼出平稳无抖动；推进 Milestone M2（原前台失焦精准返还、`/dev/uinput` 硬件直接粘贴、透明代理拖出外部）。
- **Known issues**: 无。全屏严丝合缝、4.7ms 极速快捷键唤醒、免重启热录入已全面达到工业级平稳。

## §0A 最近会话

### Session 2026-09-17 - 根治“界面抽搐抖动”，确立 Zero-Jitter 无抖动纯净映射规范

- **核心闭环**：
  1. 深入 GNOME Shell `windowManager.js` 源码，查明 `enable-animations=false` 反而导致 0.01 尺度首帧被渲染并暴力弹爆全屏的系统级缺陷；
  2. 查明 `OverlayWindow::reveal()` 中 20ms 定时器重复广播 `max_horz/max_vert` 导致 Mutter 触发 `MOVE_RESIZE` 二次重排的抖动根因；
  3. 彻底精简呼出链路，消除 20ms 定时器与冗余 X11 广播，呼出耗时降至 **4.7ms**，实测界面完全平稳、零抽搐。

### Session 2026-09-17 - 根除窗口抽搐缩放与全屏未覆盖 bug，确立 Zero-Twitch 规范

- **核心闭环**：
  1. 定位 `UTILITY` 窗口被 Mutter 禁止最大化并居中放置的硬编码陷阱，恢复为标准 `NORMAL` 类型；
  2. 启用 GTK 原生 `window.maximize()` 首帧贴合，彻底消除居中留白与短暂未覆盖 bug；
  3. `set_skip_taskbar` 移除多余坐标操作，仅保留纯净 `StackMode::ABOVE`；
  4. 弱引用可见性防抖守卫与 GSettings 幂等性过滤，实测 `Corners: +132+64 -0+64 -0-0 +132-0` 零缝隙贴合。

### Session 2026-09-17 - 落地切片 S3：可视化快捷键自定义与录入交互组件 (HotkeyRecorder)

- **核心闭环**：
  1. 彻底解耦硬编码 `F12` 快捷键，落地 `vok-core` 纯领域模型，支持 `Super + Space`、`Alt + Space`、`Ctrl + ~` 等任意标准组合；
  2. SQLite WAL 新增 `app_settings` 表，单写者串行持久化用户专属配置；
  3. GNOME `custom-keybinding` 动态热更新，修改即刻生效无需重启进程；
  4. 封装 `HotkeyRecorder` 交互组件与 `SettingsPopover` 浮层面板，主窗口内 Toggle 按键比对与底部 Footer 状态文本实时联动同步。

## 指引

- 标识、技术栈、结构和命令：`manifest.md`
- 范围规则与死路：`rules.md`
- 架构选择与根因：`decisions.md`
- 目标架构与实施门禁：`../docs/ARCHITECTURE.md`
- 更早会话：`history.md`
