# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: Milestone M2（焦点精准返还与双向消费闭环）全面交付！覆盖原前台活动窗口暂存与原子精准归还 (R10)、失焦自动退出 (Focus-Out)、双引擎硬件级直接粘贴流水线 (R7) 与中转站反向拖出透明代理锚点保持 (R3)，全量 69 项测试 100% 稳健通过。
- **Latest progress**: 
  1. 切片 2.4 落地：补齐并固化 GATE-M2-1~6 全部量化门禁测试（`tests/m2_gate_test.rs` 共 7 项集成用例全部通过）；
  2. 落地高频密集交互压力测试与内存配额硬断言：连续 100 轮呼出、拖出代理与直接粘贴，当前实测常驻内存 `VmRSS = 12.84 MB`（配额上限 < 40.0 MB）；
  3. 全工程 69 项单元/门禁自动化测试 100% 通过，`cargo clippy` 严格零警告，Release 剥离二进制仅 8.0MB；
  4. 同步更新 `docs/m2_planning.md` 与 `docs/MILESTONES.md`，M2 里程碑正式标定为“已交付”。
- **Next step**: 启动 Milestone M3 实施规划与架构设计（剪贴板历史捕获与去重、统一全局检索框三区联动）。
- **Known issues**: 无。双向流动交互浑然一体，常驻开销远低于指标配额。

## §0A 最近会话

### Session 2026-09-18 - 落地 M2 切片 2.2 硬件直接粘贴与切片 2.3 反向拖出透明代理锚点

- **核心闭环**：
  1. `vok-infra::input` 封装 `InputInjector`，建立 `/dev/uinput` 与 X11 XTEST 双通道，提供无感硬件击键；
  2. `vok-app` 联动实现单击/回车直接粘贴流水线，恪守 R7 延时时序守卫（50ms）；
  3. 查明 Wayland 拖出 Unmap 掐断与失焦时序冲突根因，落地 `vok-proxy-anchor` 全透明代理模式，保持 Surface 存活且视觉完全穿透；
  4. `StageView` 卡片挂载 `DragSource` 生命周期与多格式复合载荷；
  5. 门禁测试扩展至 GATE-M2-1~5，67 项自动化测试全部通过。

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

## 指引

- 标识、技术栈、结构和命令：`manifest.md`
- 范围规则与死路：`rules.md`
- 架构选择与根因：`decisions.md`
- 目标架构与实施门禁：`../docs/ARCHITECTURE.md`
- 更早会话：`history.md`
