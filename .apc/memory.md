# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: 落地 Milestone M2 切片 2.1（原前台活动窗口暂存与原子精准返还引擎、弱引用失焦自动退出 Focus-Out Auto-Close），实测 100% 恢复前台 Chrome 焦点，建立 M2 自动化门禁测试套件。
- **Latest progress**: 
  1. 确立并落盘 `docs/m2_planning.md`，拆解 M2 四大纵向切片与 GATE-M2-1~6 量化门禁矩阵；
  2. `vok-infra::window` 增补 `get_active_window_id()`、`restore_active_window()` 与 `get_window_title()`，采用 EWMH `_NET_ACTIVE_WINDOW (source=2)` 绕行 Mutter 防焦点窃取；
  3. `OverlayWindow` 落地呼出前活动窗口暂存、退出原子焦点归还，以及基于 `window.connect_is_active_notify` + `has_been_active` 状态守卫的失焦平滑自动关闭；
  4. 实机实测：Release 后台进程呼出捕获 `[0x400004] (MC100 Server - Google Chrome)`，隐藏瞬间精准交还焦点，`xprop -root _NET_ACTIVE_WINDOW` 权威复核为 `0x400004`；
  5. 落地 `tests/m2_gate_test.rs`，全量 56 项测试 100% 稳健通过。
- **Next step**: 推进 Milestone M2 切片 2.2（基于 Linux 内核 `/dev/uinput` 的硬件级直接粘贴与权限降级保护）。
- **Known issues**: 无。双向焦点无缝返还，失焦自动退出安全无抖动。

## §0A 最近会话

### Session 2026-09-17 - 落地 M2 切片 2.1：前台焦点暂存与原子精准返还引擎、失焦自动退出 (Focus-Out)

- **核心闭环**：
  1. 确立并落盘 `docs/m2_planning.md` 整体技术方案；
  2. `vok-infra::window` 封装 X11/EWMH `_NET_ACTIVE_WINDOW` 查询与恢复接口，单测覆盖非法 XID 容错；
  3. `OverlayWindow` 呼出时智能识别排除自身并暂存前台 XID，`hide()` 时原子触发恢复请求；
  4. 挂载激活状态通知，设计 `has_been_active` 状态守卫排除映射首帧伪失焦，实机 Toggle 与 xprop 验证焦点 100% 精准归还原应用。

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

## 指引

- 标识、技术栈、结构和命令：`manifest.md`
- 范围规则与死路：`rules.md`
- 架构选择与根因：`decisions.md`
- 目标架构与实施门禁：`../docs/ARCHITECTURE.md`
- 更早会话：`history.md`
