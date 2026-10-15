# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: 攻克纯 D-Bus 链路长按 500ms 自动关闭与蓝牙键盘权限隔离（决策 §29 / 规则 R14）；落地 DbusHoldDetector 连发心跳保活状态机（85ms 侦听判决 + 110ms silence 保活），即使无 evdev 权限也能实现长按持续常驻、松手即隐；同时重构 `/etc/udev/rules.d/70-vok-input.rules` 赋予 `TAG+="seat", ENV{ID_SEAT}="seat0"` 破解蓝牙 UHID 权限隔离；`EvdevMonitor` 增加后台动态热插拔自检；全量 81 项测试全绿，全新 Release 生产守护进程常驻上线。
- **Latest progress**: 
  1. 根因确证：用户使用的 VGN S99-2 蓝牙键盘挂载于 `/dev/input/event18`（通过 uhid），因缺失 `path_id` 导致未被 systemd 赋予 `seat0`，且非 input 组成员导致无读取权限；系统回退至纯 D-Bus 链路；
  2. 纯 D-Bus 缺陷定位：GNOME 快捷键在长按时 500ms 产生首发 repeat，旧逻辑将其误判为二次点击关闭；
  3. 落地 DbusHoldDetector：窗口可见时首发启动 85ms 侦听，85ms 内收到第二发连发判定为长按，进入 `is_holding = true` 并启动 110ms silence 保活计时器，长按过程中连发持续刷新计时器，松手后 110ms 自动隐退；短按则 85ms 超时正常隐藏；
  4. 重构 udev 规则与动态热插拔：`70-vok-input.rules` 赋予 seat0 与 uaccess，`EvdevMonitor` 增加后台动态自检扫描线程与 `scan_now`，支持热插拔与权限即刻生效；
  5. 81 项测试全绿，Release 二进制编译完成并拉起常驻守护进程。
- **Next step**: 引导用户实机测试长按快捷键（如 `Alt + Space`），验证长按过程中界面始终稳定常驻、松开按键后界面立即自动隐退，短按正常常驻呼出。
- **Known issues**: 无。

## §0A 最近会话

### Session 2026-09-20 - 攻克长按无限震荡、输入法乱切与 350ms 迟滞，实现 0ms 极致跟手与输入法完全隔离 (决策 §30 / 规则 R14)

- **核心闭环**：
  1. **查清输入法中英频繁乱切与 350ms 迟滞根因**：
     - UI 层 `key_controller` 仅拦截了 `key_pressed` 组合键，未监听 `key_released`，且放行了 `Alt_L` / `Alt_R` 单独按压；导致孤立修饰键释放事件泄露至 `search_entry` 和 Fcitx5，被输入法识别为中英切换指令；
     - 纯依赖 D-Bus 连发心跳静默超时（350ms）检测长按松手时，用户抬起手指后界面仍停滞超过 1/3 秒才消失，无法实现“手抬即隐”的瞬关体验，交互严重滞后；
  2. **0ms 极致跟手瞬关（窗口级按键释放感知）**：
     - `OverlayWindow` 挂载 `key_controller.connect_key_released`，引入 `is_initial_press_held` 状态机；
     - 窗口获取焦点后以 0ms 延迟直接捕获物理按键释放事件，若按压时长 $> 280\text{ms}$（长按瞬态），手抬即刻触发 `on_dismiss()` 隐藏窗口；若 $\le 280\text{ms}$（短按），保持常驻展示并解锁搜索框文本输入；
     - `OverlayWindow::hide()` 清场时原子重置 `is_initial_press_held = false`；
  3. **彻底隔离输入法（修饰键防泄露）**：
     - 在 `connect_key_pressed` 与 `connect_key_released` 中全面拦截并消费热键涉及的修饰键（Alt/Ctrl/Shift/Super）按下与释放，彻底阻断其进入底层 `GtkIMContext`，彻底根治 Fcitx 中英文状态跳变；
  4. **全量构建与进程上线**：全量 81 项测试全绿，Release 二进制已上线实测，用户确认“测试符合预期”。

### Session 2026-09-20 - 攻克纯 D-Bus 长按 500ms 自动关闭与蓝牙 UHID 权限隔离：DbusHoldDetector 心跳保活与动态热插拔 (决策 §29 / 规则 R14)

- **核心闭环**：
  1. **查清长按 500ms 自动关闭根因**：实体键盘为蓝牙无线键盘（`uhid` 挂载），因缺失 `path_id` 未被分配 `ID_SEAT="seat0"`，系统回退至纯 D-Bus 模式；GNOME 在物理按键按住时于 500ms 连发 `toggle`，旧逻辑误当成二次点击关闭窗口；
  2. **落地 `DbusHoldDetector` 连发心跳保活状态机**：窗口可见时首发触发 85ms 侦听，85ms 内检测到第二发连发即刻判定为长按，进入 `is_holding = true` 并启动 110ms silence 计时器；后续连发持续保活刷新计时器，松手后 110ms 超时隐退（Momentary）；短按无第二发则 85ms 超时正常隐藏（Toggle）；
  3. **重构 udev 规则与提权机制**：规则前移为 `/etc/udev/rules.d/70-vok-input.rules`（先于 71-seat.rules），显式声明 `TAG+="seat", ENV{ID_SEAT}="seat0", TAG+="uaccess", MODE="0660", GROUP="input"` 并联动 `usermod -aG input` 与 `setfacl`；
  4. **`EvdevMonitor` 动态热插拔自检**：引入 `monitored_paths` 动态集合与后台 2 秒自检线程，支持键盘设备热插拔、休眠唤醒与授权后 `scan_now` 毫秒级挂载；
  5. **全量构建与进程上线**：81 项自动化测试全绿，全新 Release 生产守护进程常驻上线。

### Session 2026-09-20 - 彻底根除长按无限交替开关震荡：GNOME Typematic 连发抑制与三重防御架构 (决策 §28 / 规则 R14)

- **核心闭环**：
  1. **查清长按无限开闭交替根因**：GNOME `custom-keybinding` 在物理长按时持续连发 `toggle`（500ms 延时 + 30ms 连发间隔）；旧代码仅对 `is_visible == true` 做了 350ms 防抖，对 `is_visible == false` 完全无防抖，导致 350ms 隐藏后 30ms 立即被下一发连发重新拉起，构成 ~380ms 周期的无限时钟振荡；
  2. **落地三重防御架构 (Triple-Shield Architecture)**：
     - 第一重：`EvdevMonitor` 暴露原子 `is_hotkey_active()`，物理按键被按住时直接丢弃所有外部 DBus `toggle`；
     - 第二重：`last_toggle_call` 记录连发间隔，$< 150\text{ms}$ 的突发连发一律抑制；
     - 第三重：`last_state_change` 全生命周期双向对称防抖，显隐两向均受 350ms 阈值保护；
  3. **压力测试与竞态消除**：模拟高并发 DBus 连发，捕获并修复了 `last_toggle_call` 的 `RefCell already borrowed` 临时借用生存期缺陷；
  4. **全量构建与进程上线**：85 项自动化测试全绿，Release 二进制常驻启动，实机验证连发抑制与正常切换均 100% 准确。


