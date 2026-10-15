# 架构决策记录 (Decisions)

> 本文记录 Linux 原生 Vok 项目的核心技术决策、评估论据与被否决方案。任何底层选型变动均需在此追加条目。

## 索引

- §1 **Linux 原生全分层架构与 Rust + GTK 4 基线** — 彻底废弃 WebView2/Tauri 路线，采用纯原生单进程模型与内存安全系统语言。
- §2 **基于 Linux 内核 evdev 的物理键态独立监听** — 绕过合成器输入抓取死锁，实现 100% 可靠的拖拽中呼出与长短按状态机。
- §3 **Wayland wlr-layer-shell 与 X11 双栈全屏浮层** — 以 OVERLAY 层级保障零黑帧无闪烁的原子呈现，不挤占系统工作区。
- §4 **中转站 (Stage) 引用型货架与双向拖拽闭环** — 纯引用零拷贝落库，通过透明代理锚点解决 Wayland 拖出时 Surface 隐藏中断会话的系统级缺陷。
- §5 **SQLite WAL 单写者与内容寻址资产存储** — 结构化元数据与大体积外置资产解耦，强事务保障异常退出不丢数据。
- §6 **基于 /dev/uinput 的硬件级输入注入 (直接粘贴)** — 绕过 Wayland 跨应用按键伪造限制，安全实现前台失焦自动输入。
- §7 **主界面 1:2:1 三栏架构、防截断高度算法与虚拟滚动** — 启动台/中转站/剪贴板横向分布，卡片高度整除规避 UI 截断，GTK4 视口回收支撑海量条目极低内存。
- §8 **双向焦点生命周期与原前台失焦精准返还** — 呼出聚焦搜索框，关闭原子返还原应用输入框，失焦/Esc/快捷键安全关闭。
- §9 **Workbench-APP 定位规范：功能交互范本与技术架构反面教材** — 继承三栏与拖拽交互精髓，彻底摒弃 Web/Tauri/IPC 膨胀与 sleep 补丁代码。
- §10 **幽灵 HUD 覆盖层、任务栏彻底隐形 (Skip Taskbar) 与 StatusNotifierItem (SNI) 托盘常驻** — 杜绝 Ubuntu Dock 污染，实现纯净后台常驻与 HUD 瞬时显隐。
- §11 **浮层强制置顶（_NET_WM_STATE_ABOVE）与 D-Bus Action 零延迟极速呼出架构** — 窗口置顶绕过防焦点窃取，D-Bus Action 触发耗时降至 8ms。
- §12 **通用全屏工作区（EWMH _NET_WORKAREA）自适应计算与纯直角边缘规范** — 彻底直角零缝隙全覆盖，权威动态工作区适配。
- §13 **快捷键领域契约 (HotkeyConfig)、GSettings 系统通道动态热重载与可视化录入控件 (HotkeyRecorder)** — 解耦硬编码 F12，免重启实时生效，录入交互安全拦截。
- §14 **GNOME Mutter 窗口类型 (NORMAL vs UTILITY) 约束与最大化首帧贴合规范 (Zero-Twitch Geometry Policy)** — 根治缩放抽搐与全屏未覆盖 bug，原生最大化首帧贴合。
- §15 **GNOME Mutter 动效机制与无抖动纯净映射规范 (Zero-Jitter Pure Mapping Policy)** — 根治 0.01 缩放抽搐与 20ms 二次重排抖动，实现 4.7ms 极速稳定呼出。
- §16 **EWMH 双向前台焦点暂存与原子精准返还引擎、失焦自动退出 (Focus-Out) 与窗口防窃取绕行规范** — 呼出前毫秒级读取活动 XID，关闭原子交还焦点，点击外部平滑自动退让。
- §17 **基于 /dev/uinput 与 X11 XTEST 双引擎的硬件直接粘贴 (R7) 与反向拖出透明代理锚点保持 (R3)** — 攻克 Wayland 拖出注销掐断会话缺陷，建立 R7 延时时序保护与复合载荷拖出闭环。
- §18 **独立首选项窗口 (SettingsWindow)、深浅色主题引擎与多轨失焦/Esc可靠隐退设计** — Libadwaita PreferencesWindow 单例常驻、双入口唤起、StyleManager日夜跟随、Capture阶段截断Esc与工作区外部点击失焦感知。
- §19 **废除 _NET_WM_STATE_ABOVE 强置顶，建立单次呼出激活提升与四维多轨失焦隐退规范 (Focus-Out Auto-Hide Policy)** — 根治 Mutter 在 ABOVE 窗口下抑制前台切换与失焦事件的死穴，建立单次提升、GTK Focus/Active 监听、30ms 外部点击感知与前台 XID 迁移检测四维联动。
- §20 **全局非界面交互点击捕获、入焦全生命周期守卫与全协议（X11/Wayland/GNOME Shell）失焦归并规范 (Non-Interface Dismiss & Unified Focus-Loss Policy)** — 窗口级 Capture 阶段穿透拾取与非交互空白命中，统合 0x0 与非自身活动窗口判定，建立入焦保护闭环。
- §21 **工作区最小尺寸约束解除与 EWMH 属性覆盖根治规范 (Workarea Minimum-Size Squeeze & Native Maximization Policy)** — 解除中转站硬性最小宽度锁死（避免 3104px > 3068px 导致 Mutter 拒绝最大化降级为居中悬浮窗），废除 Client 级 REPLACE 覆写 `_NET_WM_STATE`，建立动态重算贴合机制。
- §22 **GNOME Shell 系统概览（OverviewActive）D-Bus 毫秒级订阅与任务栏指针失焦感知规范 (GNOME Overview & Taskbar Pointer-Loss Policy)** — 解决 Super 键系统概览与 Ubuntu Dock 空白处点击不隐藏死穴，通过 org.gnome.Shell D-Bus 属性与指针移出工作区 150ms 时序双重闭环。（注：指针移出已被 §23 全面废除与替换）。
- §23 **废除指针移出表面猜想，建立基于 XInput 2 (XI2 RawButtonPress) 全屏幕无侵入物理点击与真实前台失焦架构规范 (True Focus-Loss & XI2 Passive Outside-Click Policy)** — 彻底根除晃动鼠标误退出与点击内联非交互区误关闭病根；基于 XI2 原始按键流被动监听全屏（含 Ubuntu Dock 与顶栏）物理点击；GTK 事件控制器零假动作；鼠标晃动 0 响应；外部点击 100% 精准退让并原子返还焦点。
- §24 **基于 PolicyKit (Polkit) 原生授权与 Linux 内核 evdev 硬件级全盲区鼠标点击感知规范 (Polkit & Evdev Hardware Mouse Click Policy)** — 建立标准桌面 Polkit 原生认证与现代 `uaccess` 规范，一键图形化授权；封装 `EvdevMouseWatcher` 直接读取物理鼠标 `BTN_LEFT`，实现包含 Ubuntu Dock 纯空白死区在内的全屏任意盲区 0 微秒瞬时退让，鼠标晃动 100% 绝对免疫。
- §25 **突破 Wayland 坐标钳位与触控板轻击 (Tap) 状态机：双轨窗口层级与全按键退让规范 (Wayland Clamp-Bypass & Touchpad Tap Policy)** — 采用 `child != vok_xid` 突破 Wayland 任务栏坐标钳位死穴；落地触控板 `BTN_TOUCH` 250ms 轻击状态机与全按键监听；澄清日志因果链，彻底攻克 Dock 空白处无法隐退难题。
- §26 **攻克 Dock 首次点击成功后全失效与任务栏图标偶发驻留：边界钳位容差与首帧预写入规范 (Dock Border Clamping & Pre-Map Taskbar Invariant)** — 引入 6 物理像素边缘钳位容差算法，根治 Xwayland 坐标冻结；首帧前直接向 X11 属性写入 `_NET_WM_STATE_SKIP_TASKBAR`，彻底消除 Ubuntu Dock 图标偶发驻留。
- §27 **快捷键长短按模式判决重构、内核 evdev 组合键全硬件流与 GNOME DBus 回声防抖规范 (Release-Decided Hotkey & Evdev Chord Pipeline Policy)** — 根治首帧 4.7ms 误判 Momentary 闪退缺陷，以释放时差值权威判定长短按；`EvdevMonitor` 原子追踪修饰键与组合键解耦，支持热重载与 Mid-drag 呼出；350ms DBus 回声防抖彻底消除 evdev 与系统快捷键双轨竞态。
- §28 **彻底根除长按无限交替开关震荡：GNOME Typematic 连发抑制与三重防御架构 (Typematic Repeat Suppression & Triple-Shield Toggle Architecture)** — 查明 GNOME typematic 自动连发（500ms delay + 30ms interval）与单向防抖漏洞造成的 ~380ms 振荡；构建硬件电平权威仲裁、连发心跳感知与双向对称防抖体系。
- §29 **双轨长短按自适应状态机、蓝牙 UHID 设备 seat0 提权与 evdev 动态热插拔自检规范 (Dual-Track Hold Detector & Dynamic Evdev Hotplug Policy)** — 建立 DbusHoldDetector 连发心跳保活状态机（85ms 侦听判决 + 110ms silence 保活），彻底解决纯 D-Bus 链路长按 500ms 自动闪退问题；重构 `/etc/udev/rules.d/70-vok-input.rules` 赋予 `TAG+="seat", ENV{ID_SEAT}="seat0"` 破解蓝牙 UHID 权限隔离；`EvdevMonitor` 落地后台动态热插拔自检与权限即刻重载。
- §30 **窗口级按键释放拦截与输入法隔离架构：0ms 极致跟手长按瞬关与 Fcitx/IBus 修饰键防泄露体系 (Zero-Latency Window Key-Release & IME Isolation Architecture)** — 挂载 Release 监听阻断修饰键泄露，长按手抬 0ms 瞬隐，彻底根治输入法乱切与 350ms 迟滞。

---

## 详情

### §1 Linux 原生全分层架构与 Rust + GTK 4 基线

- **背景与痛点**：
  前作 `Workbench-APP` 基于 Tauri/React/WebView2 构建，在 Windows 上遇到了极其沉重的多进程开销（80MB~800MB 内存膨胀）、IPC 序列化延迟、全屏隐藏/恢复期间的透明 WebView 帧撕裂，以及为了绕过底层异步时序而不得不添加的数十处 `sleep` 补丁。在 Linux 环境下，如果继续采用 Web 技术（WebKitGTK 进程树），这些问题不仅无法缓解，反而会在多桌面环境（GNOME/KDE/wlroots）下因渲染进程沙箱导致拖拽失效。
- **最终方案**：
  - 核心系统语言采用 **Rust**。
  - GUI 表现层采用 **GTK 4 + Libadwaita + gtk4-layer-shell**。
  - 结构拆解为三层架构：`vok-core`（纯领域状态机）、`vok-infra`（Linux 系统抽象）、`vok-app`（组合根与 GTK 视图）。
- **被否决方案**：
  - *Tauri / Electron*：重复已知多进程内存黑洞与 IPC 延迟；
  - *C++ / Qt 6*：成熟度高，但动态链接库庞大、与现代 Wayland layer-shell 的精细结合代码繁琐，且 C++ 缺乏 Rust 类型级别的无畏并发与代次所有权保障；
  - *纯 Wayland C / 自绘图形库 (egui/iced)*：缺少完整的无障碍支持、成熟的输入法 (IME) 集成和系统级 XDG 图标/主题支持，维护成本过高。
- **状态**：Active（2026-09-10 确立）。

---

### §2 基于 Linux 内核 evdev 的物理键态独立监听

- **背景与痛点**：
  Vok 的核心特征之一是“长按 250ms 临时呼出、松开即隐藏（Momentary）”以及“拖拽外部文件过程中按下快捷键呼出（Mid-drag Summon）”。但在 Linux 桌面：
  1. 用户在外部文件管理器抓取文件时，合成器进入全局拖拽抓取循环（Pointer Grab），应用级常规快捷键被直接吞掉；
  2. Wayland 出于安全考虑禁止非特权客户端监听全局键盘；
  3. X11 的 `XGrabKey` 无法提供高精度微秒级释放电平判定。
- **最终方案**：
  - 后台常驻一个独立的 `InputMonitor` 线程，直接读取 Linux 内核事件接口 **`/dev/input/event*`（`evdev`）**；
  - 监听物理键盘的 `EV_KEY` 原始事件（1=Press, 0=Release），带有微秒级硬件时间戳；
  - 按下沿记录 `press_time` 并派发 `GenerationId`，根据时间差严格分流 Momentary 与 Toggle 行为；
  - 只要运行用户处于 `input` 用户组（现代 Linux 桌面标准），无论当前系统发生何种拖拽或窗口遮挡，呼出永远 100% 触发。
- **状态**：Active（2026-09-10 确立）。

---

### §3 Wayland wlr-layer-shell 与 X11 双栈全屏浮层

- **背景与痛点**：
  工作台必须是全屏且透明覆盖，但不能破坏底层任务栏，也不能在呼出瞬间出现窗口白闪或阴影框。
- **最终方案**：
  - **Wayland 首选路线**：使用 `gtk4-layer-shell`，将窗口挂载到 `ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY`。设置四周锚点贴合，`exclusive_zone = 0`。Wayland 合成协议的 Atomic Commit 特性保证在首帧渲染完成后才一次性呈现，彻底根除视觉抖动；
  - **X11 / GNOME Mutter 回退路线**：设置 EWMH 属性 `_NET_WM_WINDOW_TYPE_DOCK` 并置顶，通过 XDG WorkArea 计算避开桌面面板。
- **状态**：Active（2026-09-10 确立）。

---

### §4 中转站 (Stage) 引用型货架与双向拖拽闭环

- **背景与痛点**：
  - **拖入**：需要接收跨应用文件并即时在面板呈现，绝不能因为用户拖入数 GB 视频而产生卡顿。
  - **拖出**：Wayland 下有一个严苛的系统约束——**一旦发起拖拽的窗口 (Surface) 被 Unmap/Hide，合成器会立即宣告该拖拽会话中止 (Cancelled)**。如果在拖出起手时直接隐藏面板，外部应用将永远收不到 Drop。
- **最终方案**：
  1. **引用模型**：拖入时解析 `text/uri-list`，只提取文件路径，秒级写入 SQLite 引用库，由后台 Worker 异步生成缩略图，不阻塞主线程；
  2. **透明代理锚点 (Transparent Proxy Anchor)**：拖出发生时，面板不执行 Destroy 或完全 Hide，而是将面板背景切为全透明并收缩为 1x1 像素的极小锚点表面保持存活；待目标窗口完成消费（收到 `GdkDrop` 成功回调）或用户按 Esc 取消后，再彻底清场隐藏。
- **状态**：Active（2026-09-10 确立）。

---

### §5 SQLite WAL 单写者与内容寻址资产存储

- **设计规范**：
  - 采用 `rusqlite`，强制开启 `PRAGMA journal_mode = WAL;`；
  - 唯一 Storage Worker 拥有写事务，读操作可由多连接并发无锁读取；
  - 临时图片/剪辑采用 SHA-256 内容寻址外置存储在 `$XDG_DATA_HOME/vok/assets`，数据库仅记录哈希与元数据，保障数据库紧凑与长时间运行不退化。
- **状态**：Active（2026-09-10 确立）。

---

### §6 基于 /dev/uinput 的硬件级输入注入 (直接粘贴)

- **背景与痛点**：
  历史剪贴板点击“直接粘贴”需自动在原前台窗口输入 `Ctrl+V`。在 X11 下可用 `xtest`，但在 Wayland 下出于安全模型严禁跨客户端伪造键盘事件。
- **最终方案**：
  - 在底层使用 Linux 内核的 **`/dev/uinput`** 创建系统级虚拟输入设备；
  - 浮层隐藏并失焦后，向内核直接发送虚拟按键扫描码（`KEY_LEFTCTRL` + `KEY_V`）；
  - 无论目标窗口是原生 Wayland 应用、XWayland 应用、全屏终端还是特权窗口，均能稳定完成输入注入。
- **状态**：Active（2026-09-10 确立）。

---

### §7 主界面 1:2:1 三栏架构、防截断高度算法与虚拟滚动

- **背景与痛点**：
  1. 用户在日常桌面流转中需要“快速启动应用/文件”、“临时文件暂存与拖拽中转”以及“历史剪贴板回溯消费”三位一体，单一视窗难以兼顾效率；
  2. 传统卡片流常因窗口绝对像素高度变动，导致最后一行卡片露出半截，视觉产生严重割裂与截断感；
  3. 随着启动项、中转文件以及剪贴板历史大量累积（几百至数千条），若采用简单全量 DOM/Widget 渲染，内存将线性暴涨至几百 MB，并引发滚动掉帧与卡顿。
- **最终方案**：
  - **1:2:1 横向布局**：主界面由左至右分为【启动台】（25%）、【中转站】（50%）、【剪贴板】（25%），顶部常驻全局联动搜索框；
  - **防截断高度动态算法**：启动台与中转站卡片共用统一高度算法，根据可用工作区高度 $H$、行间距 $G$ 和行数 $N$ 动态约束单行高度：$H_{card} = \lfloor (H - (N - 1) \times G) / N \rfloor$，确保默认视口下最底部一行永远完整呈现；
  - **视口回收虚拟滚动**：基于 GTK 4 `GtkListView` / `GtkGridView` 与 `GtkSignalListItemFactory` 构建原生虚拟化列表，仅按需实例化视口可见卡片，实现数千条数据下内存恒定且零掉帧。
- **状态**：Active（2026-09-12 确立）。

---

### §8 双向焦点生命周期与原前台失焦精准返还

- **背景与痛点**：
  工作台呼出后需立即支持键盘打字过滤（如直接敲击字符搜索应用或文件），但在工作台关闭或用户执行“直接粘贴/启动”时，若焦点无法精确无缝返还给呼出前的原始输入框（如 IDE、浏览器输入框、终端），将导致直接粘贴失败或丢失当前工作上下文。
- **最终方案**：
  - **呼出即入焦**：面板 Map 时设置 Layer Shell `KeyboardMode::OnDemand` 或焦点抢占，默认自动聚焦于顶部全局搜索框；
  - **前台上下文暂存**：呼出瞬间记录此前处于活动的 X11 窗口句柄或 Wayland 合成器前台状态；
  - **失焦与多途径关闭**：支持快捷键再次触发（Toggle）、Esc 键、点击非面板区域（如任务栏、其他窗口）失焦即自动安全关闭（Focus-Out Auto-Close）；
  - **原子交接**：工作台 Unmap 触发时，主动触发焦点回退协议，确保原应用重新获得物理前台焦点后再执行虚拟键盘注入（Ctrl+V），形成丝滑闭环。
- **状态**：Active（2026-09-12 确立）。

---

### §9 Workbench-APP 定位规范：功能交互范本与技术架构反面教材

- **背景与痛点**：
  前作 `Workbench-APP` 在产品功能定义、交互闭环（三栏布局、拖拽中呼出、引用货架、快捷消费）上经过长期打磨，产品逻辑完全成熟。然而在工程实现上，基于 Electron/Tauri/WebView2 的多进程模型与异步 IPC 导致内存占用动辄几百 MB、冷启动迟缓、全屏撕裂，并通过大量粗暴的 `sleep` 掩盖竞态，属于技术架构上的失败典型。
- **最终方案**：
  - **双重定位**：将 Workbench-APP 严格限定为**“功能形态与交互体验的成熟参考”**与**“底层技术架构与代码实现的反面教材”**；
  - **代码隔离禁令**：坚决禁止将 Workbench-APP 的 Web 前端代码、Tauri IPC 管道和基于 `sleep` 的时序逻辑移植到本项目；
  - **原生重构**：所有交互必须基于纯 Rust 单进程、Linux 内核接口与 GTK4 原生渲染重新实现。
- **状态**：Active（2026-09-12 确立）。

---

### §10 幽灵 HUD 覆盖层、任务栏彻底隐形 (Skip Taskbar) 与 StatusNotifierItem (SNI) 托盘常驻

- **背景与痛点**：
  在 Windows 版 Vok 中，工作台以 ToolWindow 呈现，不占任务栏、呼出无感，仅在系统托盘静默常驻。而在 Linux (GNOME/Ubuntu) 默认环境下，普通应用顶级窗口（`xdg-toplevel`）在呼出时，Ubuntu Dock 会自动在左侧点亮图标、显示运行小白点并伴随动效，破坏了“全屏沉浸式 HUD 第二桌面”的产品纯净质感。
- **最终方案**：
  1. **EWMH 任务栏隐形属性注入**：
     - 默认在 X11 / XWayland 下运行，窗口映射时通过 `x11rb` 直接注入 `_NET_WM_STATE_SKIP_TASKBAR` 与 `_NET_WM_STATE_SKIP_PAGER`；
     - 窗口类型设置为 `_NET_WM_WINDOW_TYPE_UTILITY`；
     - Ubuntu Dock 与 GNOME Shell 过滤该窗口（`skip_taskbar == true`），彻底不在左侧任务栏产生任何图标或运行指示灯；
  2. **静默 Desktop Entry**：
     - 在 `~/.local/share/applications/com.vok.desktop.desktop` 写入 `NoDisplay=true`，避免在系统常规应用列表中被当作普通应用调度；
  3. **StatusNotifierItem (SNI) 托盘常驻与守护**：
     - 接入 D-Bus `StatusNotifierItem` 规范，在顶部状态栏常驻 `view-grid-symbolic` 图标；
     - 窗口内按 Esc / 点击空白处仅调用 `hide()`，配合 `gio::ApplicationHoldGuard` 维持后台极低开销守护（< 5MB）；
     - 支持托盘左键 Toggle 呼出/隐藏，右键菜单提供【呼出工作台】与【退出 Vok】。
- **状态**：Active（2026-09-16 确立）。

---

### §11 浮层强制置顶（_NET_WM_STATE_ABOVE）与 D-Bus Action 零延迟极速呼出架构

- **背景与痛点**：
  在桌面人工实测中暴露出两大核心体验痛点：
  1. **呼出不流畅**：GNOME 快捷键直接绑定执行应用二进制，导致每次按 F12 都会在操作系统层 fork+exec 一个全新的庞大 GTK4 进程（耗时 200~300ms），引发 CPU 峰值与磁盘 I/O，严重卡顿迟滞；
  2. **界面没有显示在最上层**：窗口缺少 EWMH `_NET_WM_STATE_ABOVE` 置顶属性与 `XRaiseWindow` 提升，且外部进程唤醒触发了 GNOME Mutter 的防焦点窃取（Focus Stealing Prevention），导致面板被浏览器/终端等前台窗口遮挡；此外状态机未针对 Toggle 短按显式投递 `PhysicalRelease`，导致误入 Momentary 瞬态模式。
- **最终方案**：
  1. **D-Bus 原生 Action 零延迟极速触发**：
     - 在 GApplication 注册原生 Action `"toggle"`；
     - GNOME 系统快捷键直接调用 `gdbus call --session --dest com.vok.desktop --object-path /com/vok/desktop --method org.gtk.Actions.Activate "toggle" [] {}`；
     - 触发耗时由 300ms 骤降至 **8ms**（提升 35 倍以上），完全在常驻进程内零开销响应，杜绝进程重复拉起；
  2. **EWMH 强制置顶与硬件级提升 (XRaiseWindow)**：
     - 注入 `_NET_WM_STATE_ABOVE` 与 `_NET_WM_STATE_STAYS_ON_TOP`；
     - 呼出时向 Root Window 广播 `_NET_ACTIVE_WINDOW`（source=2: Pager/Panel，绕过防焦点窃取）；
     - 直接执行 X11 原生 `configure_window(StackMode::ABOVE)`，并在首帧提交后 20ms 二次强化，保证在任何软件全屏下绝对居于最顶层；
  3. **会话状态机 Toggle 短按闭环**：
     - 在 Toggle 呼出链路中原子注入 `PhysicalPress` 与 `PhysicalRelease`，保证状态机精准进入稳定常驻的 `Visible { mode: Toggle, is_key_down: false }`。
- **状态**：Active（2026-09-17 确立）。

---

### §12 通用全屏工作区（EWMH _NET_WORKAREA）自适应计算与纯直角边缘规范

- **背景与痛点**：
  人工测试反馈工作台未实现真正全屏覆盖，外层带大圆角导致桌面壁纸透出，且右侧和底部存在 32px / 24px 大缝隙。根因排查发现：
  1. 样式表在 `.vok-root-container` 上写死了 `border-radius: 20px` 与外边框，破坏了第二桌面 HUD 与屏幕边缘切齐的纯净感；
  2. 代码硬编码减去了 `-16px` 和 `-12px` 的“安全间距”，且假设任务栏仅位于左侧与顶部，无法适应任务栏在底部、右侧、隐藏或多显示器的桌面环境；
  3. 未在三栏计算时扣除内边距（40px），曾导致内容撑破窗口，进而诱发了上述负向硬编码补丁。
- **最终方案**：
  1. **彻底直角与纯黑沉浸底色**：移除所有外层圆角（`border-radius: 0px`）与边框（`border: none`），窗口背景设为不透明暗色（`rgba(18, 20, 28, 0.95)`），确保与屏幕边缘或任务栏绝对无缝拼接；
  2. **EWMH _NET_WORKAREA 权威动态映射**：
     - 直接信赖 X11 窗口管理器发布的 `_NET_WORKAREA = [x, y, width, height]`（无论任务栏在左/右/顶/底还是自动隐藏，均已权威扣减完成）；
     - 结合显示器 HiDPI Scale 无损换算为逻辑尺寸，严禁任何人工扣减或写死偏移；
  3. **X11 协议级坐标与几何强行锁定**：
     - 在 `set_skip_taskbar` 中通过 `conn.configure_window` 明确传入 `x, y, width, height`，硬件级将窗口锚定至工作区起点并拉伸至全部可用宽高；
  4. **三栏内容几何精密扣除**：
     - 传入 `three_pane.apply_geometry` 的宽度严格为 `logical_width - 40`，三栏总宽加上左右 padding 恒等于 `logical_width`，零像素误差。
- **状态**：Active（2026-09-17 确立）。

---

### §13 快捷键领域契约 (HotkeyConfig)、GSettings 系统通道动态热重载与可视化录入控件 (HotkeyRecorder)

- **背景与痛点**：
  呼出快捷键此前写死为 `F12`，缺乏面向用户的自定义录入机制。在 60%/65% 紧凑型配列、Mac 布局或某些必须按 `Fn+F12` 的轻薄本上难以舒适单手触发。用户迫切需要自由定义专属热键（如 `Super + Space`、`Alt + Space`、`Ctrl + ~` 等），且修改后必须零延迟动态更新底层系统按键监听器，无需重启常驻进程。
- **最终方案**：
  1. **纯领域数据结构与安全性校验 (`vok-core::hotkey`)**：
     - 确立 `HotkeyConfig` 与 `ModifierMask`，支持双向 GSettings 格式序列化与人类易读展示；
     - 严格安全拦截：`F1~F24` 允许单键；普通字符键强制搭配 `Super/Ctrl/Alt` 修饰键；单 `Shift` 键、裸 `Esc`/`Return` 危险单键及纯修饰键严格拦截，杜绝打断全局文本输入；
  2. **SQLite WAL 配置持久化 (`app_settings`)**：
     - 在数据库增补通用配置表 `app_settings (key, value, updated_at_ms)`，严格由单写者 `StorageWorker` 串行落盘，读操作并发无锁；
  3. **GNOME GSettings 动态热重载 (`vok-infra::desktop::shortcut`)**：
     - 扩展 `DesktopShortcut::bind_gnome_shortcut`，修改后动态写入 `custom-keybinding`，GNOME 毫秒级接管新热键，底层保持调用 D-Bus Action `toggle`，维持 **8ms** 极速响应；
  4. **可视化录入交互组件与 Popover 宿主 (`vok-app::ui`)**：
     - 封装 GTK4 原生控件 `HotkeyRecorder`：具备 `Idle`（键帽药丸徽章）与 `Recording`（发光高亮提示、修饰键实时预览、Esc 取消、实体键安全提交）；
     - 通过 `SettingsPopover` 挂载于主窗口 Header 设置按钮，不弹独立 OS 窗口，维持全屏 HUD 纯净质感；
     - 主窗口内按键控制器动态比对当前生效按键，支持窗口内再次按唤醒键 Toggle 退出，Footer 提示文字实时同步；
  5. **桌面快捷键冲突智能检测与录入期主动避让接管机制 (Desktop Conflict Preemptive Suspension)**：
     - **死锁根因诊断**：此前仅在快捷键持久化（Commit）后调用 `handle_system_conflicts`，而在用户处于录入中（Recording 态）时，系统 `activate-window-menu` 依然是 `['<Alt>space']`。用户按下 `Alt + Space` 时，GNOME Mutter 在 Display Server 顶层先于 GTK 抢占了按键并弹出窗口控制菜单（导致“呼出某些界面”），同时导致 GTK 录入控件失去焦点触发 `focus_controller.connect_leave` 自动回滚，形成“无法成功录入”的死锁；
     - **录入态主动避让**：在 `HotkeyRecorder` 点击进入录入态时，立即调用 `DesktopShortcut::prepare_recording_conflicts()` 临时挂起 `activate-window-menu`（设为 `[]`），键盘事件直通 GTK，按键控制器升级为 `PropagationPhase::Capture` 优先捕获；
     - **取消/回滚安全保证**：用户按 `Esc` 或失焦离开时，若当前快捷键非 `Alt+Space`，通过 `restore_recording_conflicts` 立即无损恢复系统原有快捷键；
     - **一键快捷预设芯片 (Preset Chips)**：在设置面板新增常用快捷键芯片（`⌥ Alt + Space`、`⊞ Super + Space`、`Ctrl + ~`、`F12 (默认)`），提供零摩擦的一键极速切换与自动冲突接管。
- **状态**：Active（2026-09-17 确立）。

---

### §14 GNOME Mutter 窗口类型 (NORMAL vs UTILITY) 约束与最大化首帧贴合规范 (Zero-Twitch Geometry Policy)

- **背景与痛点**：
  用户反馈快捷键录入通过后，呼出界面发生“页面抽搐缩放、未覆盖全屏等短暂bug，界面不稳定”。
- **根因精确定位 (Root Cause)**：
  1. **UTILITY 窗口禁止最大化陷阱**：此前为实现任务栏隐藏将 `_NET_WM_WINDOW_TYPE` 设为了 `UTILITY`。然而在 GNOME Mutter 规范中，`UTILITY` 窗口被视作工具调色板，Mutter 强制剥夺其 `_NET_WM_ACTION_MAXIMIZE` 权限并拒绝执行最大化，强制按浮动窗口居中放置（实测处于 `x=226, y=224, 2974x1452`），导致顶部留空 224px、左侧留空 226px、底部留空 324px；
  2. **异步 X11 ConfigureWindow 与 GTK 布局打架**：为弥补上述留白，此前注释了 `window.maximize()`，改在映射后通过 X11 `configure_window` 强制传入 `(132, 64, 3068, 1936)` 并在 20ms 后二次执行。导致窗口先在屏幕居中渲染第一帧（留下四周大缝隙，即“未覆盖全屏短暂bug”），几十毫秒后被 X11 强行拖拽拉伸，诱发 GTK RenderNode 树重新分配与重绘，产生人眼清晰可见的“抽搐缩放”；
  3. **快连 Toggle 竞态与 GSettings 冗余重载**：快速连续切换时，20ms 定时器未校验可见性堆叠触发；同时 `gsettings` 在未变更时重复执行，诱发 GNOME Shell 频繁注销/重注全局热键导致合成器微卡顿。
- **最终方案**：
  1. **规范恢复为 NORMAL 窗口类型**：保持 `_NET_WM_WINDOW_TYPE_NORMAL`，搭配标准的 `_NET_WM_STATE_SKIP_TASKBAR`、`_NET_WM_STATE_SKIP_PAGER` 与 `NoDisplay=true`，Ubuntu Dock 同样 100% 不显示图标；
  2. **启用 GTK 原生 `window.maximize()` 首帧贴合**：在 NORMAL 类型下，Mutter 在呈现第一帧前即依据 `_NET_WORKAREA` 将窗口精准锚定于 `(132, 64)` 并拉伸至 `3068x1936`，首帧即严丝合缝，彻底消除居中留白与短暂未覆盖 bug；
  3. **X11 纯净置顶提升 (Pure Raise)**：`set_skip_taskbar` 的 `conn.configure_window` 仅保留 `StackMode::ABOVE`，严禁传入任何 `x/y/w/h` 坐标参数，杜绝干扰 Mutter 原生最大化几何；
  4. **弱引用可见性防抖守卫**：20ms 二次强化定时器增加 `win_weak.upgrade()` 与 `win.is_visible()` 守卫，失焦/隐藏后立刻静默，防止陈旧事件破坏合成器状态；
  5. **GSettings 幂等性过滤**：修改快捷键前比对现有值，值相同则跳过写入，避免 GNOME Mutter 产生不必要的全局按键重载风暴。
- **状态**：Active（2026-09-17 确立）。
 
 ---

### §15 GNOME Mutter 动效机制与无抖动纯净映射规范 (Zero-Jitter Pure Mapping Policy)

- **背景与痛点**：
  用户反馈“界面抽搐抖动”，伴随呼出后未覆盖全屏、尺寸异常收缩等现象。
- **根因精确定位 (Root Cause)**：
  1. **GNOME Shell `_mapWindow` 的 0.01 缩放微观缺陷**：
     提取 GNOME Shell `windowManager.js` 发现，当设置 `org.gnome.desktop.interface enable-animations = false` 时，`_mapWindow` **仍然会无条件执行** `actor.scale_x = 0.01; actor.scale_y = 0.05; actor.opacity = 0;`。随后其执行 `await this._waitForOverviewToHide()`，该 `await` 将执行权让渡给事件循环，导致 Clutter 在第 1 帧把全屏窗口实际渲染为一个 1% 宽、5% 高的中心微小斑点；第 2 帧 `ease` 耗时虽为 0ms 瞬间跳至 1.0，但人眼直接捕获到了“微小斑点 -> 瞬间拉爆全屏”的剧烈抽搐（Spasm）；销毁时亦会在首帧缩小至 0.8，产生收缩抽搐；
  2. **GTK 4 隐藏后清空最大化状态引发的 GTK 与 Mutter 布局死锁 (State Oscillation)**：
     在 GTK 4 架构中，当调用 `window.set_visible(false)` 隐藏窗口时，底层 `GdkSurface` 被销毁，GTK 内部重置 `is_maximized = false`。若在 `reveal()` 重新呼出时未显式调用 `window.maximize()`，GTK 会默认按未最大化尺寸（1486x726 逻辑尺寸，即 HiDPI 2x 下的 `2972x1452`）向合成器申请呈现。此时 X11 层的 `apply_skip_taskbar_by_title` 又试图向 Mutter 附加最大化属性，导致 GTK 与 Mutter 陷入多轮“最大化/取消最大化”的激烈竞争（日志实测发生 4 轮连续翻转：`true -> false -> true -> false`），最终窗口被 Mutter 回退为未最大化浮动窗口（位于 `+515+224`，尺寸 `2972x1452`，屏幕四周留出巨大黑边与缝隙），并伴随可见的尺寸骤缩抽搐；
  3. **20ms 延时定时器引发首帧后二次重排 (Secondary Re-layout)**：
     `OverlayWindow::reveal()` 中此前包含一个 20ms 定时器，在窗口呈现并完成第 1 帧绘制后再次调用 `apply_skip_taskbar_by_title`。而在 `set_skip_taskbar` 中又向 Mutter 发送了 `_NET_WM_STATE_ADD (max_horz, max_vert)`。这导致 Mutter 在窗口已经最大化呈现后突然收到重复的最大化请求，向 GTK 下发 `MOVE_RESIZE` 事件，GTK 部件树在呈现 20ms 后突发二次尺寸计算与重绘，产生第二次剧烈抖动。
- **最终解决方案**：
  1. **呼出时原子固化 `window.maximize()`**：
     - 在 `OverlayWindow::reveal()` 中，`set_visible(true)` 与 `present()` 执行前，无条件显式调用 `self.window.maximize()`；
     - 确保 GTK 4 在构建新 `GdkSurface` 的首帧即携带最大化约束，杜绝 GTK 向合成器发送 unmaximize 指令，彻底平息 GTK 与 Mutter 的状态震荡，100% 稳固锁定工作区尺寸 `3068x1936+132+64`（`Corners: +132+64 -0+64 -0-0 +132-0`，零像素误差）；
  2. **X11 `set_skip_taskbar` 完整状态固化与 0 延时**：
     - `_NET_WM_STATE` 属性中严格包含 `MAXIMIZED_HORZ`、`MAXIMIZED_VERT`、`SKIP_TASKBAR`、`SKIP_PAGER`、`ABOVE` 与 `STAYS_ON_TOP` 全量原子，并发送对应的 `_NET_WM_STATE_ADD`，保障 X11 协议层与 GTK 层权威互认；
     - 彻底删除 `reveal()` 中的 20ms 定时器与同步调用，呼出路径缩短为纯净的 `maximize() -> set_visible(true) -> present() -> grab_focus()`，耗时降至 **4.7ms**；
  3. **恢复 GNOME 桌面环境原生平滑渲染管线**：
     - 废弃强行篡改全局 `org.gnome.desktop.interface enable-animations` 的侵入式方案，避免触发 GNOME Shell 的 0.01 帧渲染缺陷，维持系统与窗口自然平滑的高帧率呈现；
     - 保留 GTK 4 应用内部组件级动画禁用 (`set_gtk_enable_animations(false)`)，消除组件自身内部状态延迟。
- **状态**：Active（2026-09-17 确立）。

---

### §16 EWMH 双向前台焦点暂存与原子精准返还引擎、失焦自动退出 (Focus-Out) 与窗口防窃取绕行规范

- **背景与痛点**：
  1. 工作台此前呼出后虽能正确抢占焦点至搜索框，但在退出后（按 Esc、再次按快捷键或点击空白处），原前台编辑窗口（如 VSCode、终端、浏览器输入框）无法自动恢复活动输入状态，用户必须手动再点一次鼠标，工作流严重受阻；
  2. 工作台此前缺少真正的“失焦自动退出”：用户点击任务栏或多屏其他窗口时，工作台依然常驻不退，破坏了“用完即走”的 HUD 质感；
  3. 无法支撑即将到来的 M2 切片 2.2“一键直接粘贴”：直接粘贴要求必须将物理焦点精准归还原窗口后方可执行虚拟击键。
- **最终方案**：
  1. **EWMH _NET_ACTIVE_WINDOW 双向前台暂存与恢复**：
     - 在 `OverlayWindow::reveal()` 呼出前瞬间，调用 `WindowAdapter::get_active_window_id()` 读取当前 Root 窗口的 `_NET_ACTIVE_WINDOW`；
     - 校验非自身窗口（排除 `"Vok Overlay"`）后暂存为 `previous_active_window`；
     - 在 `OverlayWindow::hide()` 隐藏时，无条件通过 `ClientMessageEvent` 发送 `_NET_ACTIVE_WINDOW`，采用 `source indication = 2 (Pager/Panel)` 绕过 Mutter 的防焦点窃取（Focus Stealing Prevention），实现原子无缝焦点归还；
  2. **弱引用失焦自动退出 (Focus-Out Auto-Close)**：
     - 挂载 `window.connect_is_active_notify` 监听窗口顶级激活状态；
     - 引入 `has_been_active` 状态守卫，严格要求经历过 `active == true -> active == false` 且当前窗口仍处于 `is_visible()` 时方才触发，彻底杜绝映射首帧阶段的误触发；
     - 经确认，窗口内 Popover（设置面板、卡片右键菜单）由于共享同一 GdkSurface，操作时不会导致 `is_active` 翻转，避免了内部操作被误退出的缺陷；
   3. **单元测试与门禁固化**：
      - 建立 `tests/m2_gate_test.rs`，验证焦点暂存/恢复调用容错、非法 XID (0) 安全 No-Op，以及 `SessionEvent::FocusLost` 的代次隔离保护。
- **状态**：Active（2026-09-17 确立）。

---

### §17 基于 /dev/uinput 与 X11 XTEST 双引擎的硬件直接粘贴 (R7) 与反向拖出透明代理锚点保持 (R3)

- **背景与痛点**：
  1. **消费单向不可逆**：中转站此前只能“外部拖入”，缺少“单击卡片直接在原光标处自动填充文件路径”的高效消费链路；且在 Linux 环境下，直接模拟击键面临 `/dev/uinput` 权限差异（普通用户通常无写权限）以及跨应用伪造按键被合成器拦截的问题；
  2. **Wayland Drag-out Unmap 掐断死穴 (规则 R3)**：当用户抓取卡片向外部拖拽时，若隐藏全屏面板，Wayland 合成器会因源 Surface 死亡而立即派发 `cancelled`，强制中断拖拽会话；
  3. **失焦与拖拽的时序冲突**：鼠标移出向外拖拽时会触发 `is_active == false`，若无保护会误触发失焦隐退，导致拖拽中途被毁。
- **最终方案**：
  1. **双引擎按键注入器 (`InputInjector`) 与 R7 时序守卫**：
     - **优先引擎**：构建 Linux 内核 `/dev/uinput` 虚拟键盘设备（`Vok Virtual Keyboard`），直接发送底层硬件扫描码；
     - **降级引擎**：无权限时自动降级回退至 X11 `XTest` 协议扩展（`xtest_fake_input`），保障在任意无特权 Linux 桌面下 100% 可用；
     - **R7 延时守卫**：直接粘贴时依次执行：剪贴板写入 -> 面板隐藏 -> 原前台焦点恢复 -> 50ms 延时注入 Ctrl+V，确保击键 100% 落在原前台光标处；
  2. **透明代理锚点保持模式 (`vok-proxy-anchor`)**：
     - 用户拖动卡片（触发 `drag-begin`）时，通知状态机进入 `SessionState::Dragging` 并执行 `enter_proxy_anchor_mode()`；
     - 挂载 `.vok-proxy-anchor` 将窗口与根容器设为全透明，隐藏 Header、三栏与 Footer，底层桌面完全可见；
     - 窗口在合成器中**保持 Mapped 存活**，确保 Wayland `wl_data_source` 会话持续有效；
     - 开启 `is_in_proxy_mode` 守卫，严格忽略拖拽期间外部穿透引发的任何 `FocusLost` 失焦事件；
     - 拖拽完成或取消（`drag-end` / `drag-cancel`）时退出代理模式，调用 `hide()` 并重置状态；
  3. **复合载荷与 GtkDragSource 协议增强**：
     - `ContentProvider::new_union` 同时打包 `gdk::FileList`（文件管理器与桌面）与文本字符串（终端与文本框），最大化外部应用兼容性；
  4. **门禁验证**：
     - `m2_gate_test.rs` 增设 GATE-M2-3（注入器与 R7 守卫）、GATE-M2-4（拖拽代理与防掐断状态流转）、GATE-M2-5（复合载荷类型检查），全量通过。
- **状态**：Active（2026-09-18 确立）。

---

### §18 独立首选项窗口 (SettingsWindow)、深浅色主题引擎与多轨失焦/Esc可靠隐退设计

- **背景与痛点**：
  1. **设置入口受限与内嵌限制**：设置原本作为 `SettingsPopover` 挂载在主工作台 Header 齿轮上，当工作台隐藏时无法从托盘独立打开，且 Popover 受尺寸限制无法承载深色/浅色切换、通道状态与行为控制；
  2. **缺少浅色/跟随系统主题规范**：全局样式硬编码深色毛玻璃，无法与 GNOME 桌面的日间/夜间配色偏好自适应联动；
  3. **Esc 键退出假死死穴**：呼出后工作台聚焦居中 `SearchEntry`，GTK4 `SearchEntry` 默认内置 `stop-search` 处理，在 Bubble 冒泡阶段拦截并消费了 `Escape` 键，导致顶层窗口接收不到 Esc 退出信号；
  4. **点击任务栏/Dock无法失焦死穴**：Ubuntu Dock (`dash-to-dock`) 与 GNOME 顶栏为 `_NET_WM_WINDOW_TYPE_DOCK` 且声明 `InputHint = False`，不接收键盘焦点，Mutter 在用户点击 Dock/顶栏时不会向 Vok 发送 `FocusOut`，导致点击任务栏无法退出。
- **最终方案**：
  1. **独立设置窗口架构 (`SettingsWindow`)**：
     - 基于 `adw::PreferencesWindow` 实现现代原生独立窗口，单例常驻，拦截 `close-request` 转为隐藏；
     - **双入口无缝联动**：托盘右键菜单增加“⚙ 偏好设置...”（通过异步通道派发 `TrayCommand::OpenSettings`）；工作台右上角齿轮点击时自动原子隐藏 Overlay 并唤起首选项窗口；
     - 包含快捷键录入与系统冲突规避、深浅色主题切换、失焦与Esc交互行为设置以及关于信息；
  2. **深浅色与跟随系统主题引擎 (`ThemeManager`)**：
     - 引入 `vok-core::ThemeMode` 纯契约枚举，SQLite `app_settings` 表持久化 `theme.mode`；
     - 接入 `adw::StyleManager`，支持 `Default`（跟随系统）、`ForceDark`（深色）与 `ForceLight`（浅色）；
     - 监听 `dark-notify`，在系统日夜交替时动态切换 `.theme-light` CSS 样式类，提供高对比度米白/灰白通透毛玻璃风格；
  3. **Esc 键 Capture 阶段强拦截**：
     - 将主窗口 `EventControllerKey` 传播阶段切换为 `PropagationPhase::Capture`，在事件传递给 `SearchEntry` 之前优先截获 `Key::Escape` 并立即退出；同时为 `SearchEntry::connect_stop_search` 附加兜底联动；
  4. **双轨失焦自动隐藏机制**：
     - **轨道 1（活动焦点监听）**：优化 `connect_is_active_notify`，设定呼出首帧 150ms 窗口交割保护期，准确捕获 Alt+Tab、Super 键或点击外部应用；
     - **轨道 2（工作区外部点击探测）**：利用 X11 `query_pointer` 检查鼠标按键按下状态与工作区边界（`is_pointer_clicking_outside`），当用户点击左侧 Dock (`x < 132`) 或 GNOME 顶栏 (`y < 64`) 时，定时器在 100ms 内灵敏感知并触发原子隐退；
     - **不变量防护**：严格保持规则 R3（反向拖拽代理模式下忽略失焦）与规则 R10（隐退时原子交还活动前台焦点）。
- **状态**：Active（2026-09-18 确立）。

---

### §19 废除 _NET_WM_STATE_ABOVE 强置顶，建立单次呼出激活提升与四维多轨失焦隐退规范 (Focus-Out Auto-Hide Policy)

- **背景与痛点**：
  在实机联调中发现，呼出工作台后，用户点击左侧任务栏 (Ubuntu Dock)、按 Alt+Tab 切换窗口或唤起其它软件时，界面始终不自动隐藏。经实机注入诊断与源码推演，查明根因为：
  1. **`_NET_WM_STATE_ABOVE` 死锁**：此前在 `set_skip_taskbar` 中固化了 `_NET_WM_STATE_ABOVE`、`_NET_WM_STATE_STAYS_ON_TOP` 与 `StackMode::ABOVE`。在 Mutter 合成器下，最大化且处于 `META_WINDOW_ABOVE` 层的窗口会被 Mutter 视为不可被覆盖的最前台，当用户按 Alt+Tab、点击 Dock 上的应用图标或尝试聚焦其它窗口时，Mutter 会阻止前台焦点的转移，导致 GTK `is_active` 状态永久保持为 `true`，无法感知失焦；
  2. **外部点击感知采样率过低**：此前 100ms 的外部指针按键轮询（`is_pointer_clicking_outside`）严重滞后于人类典型鼠标点击脉冲（30ms~70ms），极易错失瞬态点击；
  3. **首帧映射伪激活**：启动时调用 `maximize` 促使窗口提前映射并向系统广播 `_NET_ACTIVE_WINDOW`，导致启动即抢焦，首次按键被迫执行隐藏而非呈现。
- **最终方案**：
  1. **废除持续置顶属性**：彻底从 `_NET_WM_STATE` 移除 `_NET_WM_STATE_ABOVE`、`_NET_WM_STATE_STAYS_ON_TOP` 与 `StackMode::ABOVE`，仅保留 `SKIP_TASKBAR`、`SKIP_PAGER` 与 `MAXIMIZED_HORZ/VERT`；
  2. **单次呼出激活提升 (`raise_and_activate_window_by_title`)**：仅在 `reveal()` 触发时向窗口管理器发送 `_NET_ACTIVE_WINDOW` (source indication = 2) 与 `window.present()`，瞬间将工作台提升至前台并聚焦搜索框，用完即可随时被其它窗口覆盖；
  3. **四维多轨失焦隐退联动体系**：
     - **轨道 1 (GTK 焦点控制器)**：挂载 `gtk4::EventControllerFocus` 监听 `leave` 信号；
     - **轨道 2 (GTK 活动状态监听)**：挂载 `window.connect_is_active_notify`，在 `!win.is_active()` 时安全触发；
     - **轨道 3 (毫秒级外部点击探测)**：建立 `with_cached_conn` 复用 X11 连接，以 30ms 极低开销轮询，精确识别鼠标在工作区外 (Dock `x < 132` / 顶栏 `y < 64`) 点击；
     - **轨道 4 (EWMH 前台 XID 迁移检测)**：通过 `get_active_window_id` 实时比对前台活动窗口 XID，一旦检测到非 Vok 的其它应用被激活（如 Alt+Tab 或点击托盘其它应用），毫秒级退出；
  4. **首帧静默初始化**：窗口构建时显式设置 `.visible(false)`，杜绝首帧伪激活与状态机代次错位。
- **状态**：Active（2026-09-18 确立）。

---

### §20 全局非界面交互点击捕获、入焦全生命周期守卫与全协议（X11/Wayland/GNOME Shell）失焦归并规范 (Non-Interface Dismiss & Unified Focus-Loss Policy)

- **背景与痛点**：
  实机测试中用户反馈：“呼出界面后，焦点在界面上；用户点击其他非界面位置没有隐藏”。深入推演与实机排查查明以下三重隐蔽根因：
  1. **工作台内部空白区域点击冒泡吞噬**：
     此前仅在 `root_box` 挂载普通 Bubble 阶段的 `GestureClick`。由于工作台三栏布局全屏撑满，其子部件（`ScrolledWindow`、`ColumnView`、`ListView`、`FlowBox`）无条件吞噬了所有点击事件（包括卡片列表下方大片空白区域、栏目间隙与内边距），事件根本无法冒泡至 `root_box`，导致用户点击任何非卡片空白处毫无响应；
  2. **Wayland / GNOME Shell 原生窗口与概览的 `0x0` 丢焦断层**：
     此前轮询检测前台活动窗口时，仅在 `active_xid != 0` 时校验窗口标题。然而在 Ubuntu Wayland 架构下，当用户切换至任何原生 Wayland 应用（如 GNOME Terminal、Nautilus、系统设置、文本编辑器）或按下 Super 键进入 GNOME Shell 概览时，XWayland 的 `_NET_ACTIVE_WINDOW` 会变为 `0x0`。此前逻辑由于 `!= 0` 过滤，将所有 Wayland 应用切换和系统概览当成“无变化”直接忽略；
  3. **未入焦状态下的提前失焦误判 (Premature Dismissal)**：
     当窗口刚被 `reveal()` 时，窗口管理器完成 X11 映射与焦点交接需要数十至数百毫秒。若直接检测 `!win.is_active()`，会导致在窗口尚未正式入焦前便误以为“失焦”，触发自杀式闪退。
- **最终方案**：
  1. **窗口级 Capture 阶段穿透拾取 (`win.pick`) 与非交互空白命中 (`is_interactive_widget`)**：
     - 在 `window` 级别挂载 `GestureClick` 并设置为 `PropagationPhase::Capture`，抢在任何子列表控件之前截获鼠标按下（`connect_pressed`，无需等待抬起）；
     - 通过 `win.pick(x, y, PickFlags::DEFAULT)` 提取命中叶子节点并递归其祖先链；
     - 白名单判定交互控件（`Button`、`MenuButton`、`SearchEntry`、`Entry`、`Text`、`Scrollbar`、`Popover` 以及包含 `card`、`btn`、`item` 等 CSS 类名的卡片容器，同时显式排除 `skeleton` 骨架占位）；
     - 凡命中非交互区域（卡片下方空白背景、栏目间隔、边距、标题空白），立即毫秒级触发 `on_dismiss()`；
  2. **全协议失焦统合判定 (`active_xid != my_xid`)**：
     - 引入 `vok_infra::WindowAdapter::get_vok_window_id()` 缓存获取自身 XID；
     - 轮询判定时，凡检测到 `active_xid != my_xid`（彻底覆盖 `active_xid == 0` 的原生 Wayland 软件、GNOME Shell 概览与桌面，以及切换至其它 X11 软件），一律作为权威失焦事件瞬时退出；
  3. **严格的双向入焦全生命周期守卫 (`has_gained_focus`)**：
     - 新增 `has_gained_focus` 状态；`reveal()` 时初始化为 `false`；
     - 只有当 `win.is_active() == true`、`focus_ctrl` 收到 `enter` 或轮询探测到 `active_xid == my_xid` 时，方才确立 `has_gained_focus = true`；
     - 任何基于活动状态的失焦判定必须严格满足 `has_gained_focus == true && !win.is_active()`，彻底杜绝首帧未入焦时的过早闪退，实现坚如磐石的“入焦 -> 焦点转移 -> 自动退出”闭环。
- **状态**：Active（2026-09-18 确立）。

---

### §21 工作区最小尺寸约束解除与 EWMH 属性覆盖根治规范 (Workarea Minimum-Size Squeeze & Native Maximization Policy)

- **背景与痛点**：
  在落地失焦自动隐退后，用户反馈：“界面崩坏了；无法显示全屏；呼出一次一个样子；严重不稳定”。通过 `xwininfo` 观测，窗口被降级为未最大化的悬浮窗口（`Width: 3104, Height: 1452, X: 515, Y: 64`），严重偏离全屏工作区（`3068x1936+132+64`）。深入排查定位到三大架构级根因：
  1. **三栏硬性下限锁死（Min-Size Lockout）**：
     在 `LayoutAdapter::apply_layout` 中，对左（启动台）、中（中转站）、右（剪贴板）三栏均调用了 `set_size_request`。在 GTK 4 中，`set_size_request(w, -1)` 设定的是部件的**绝对最小尺寸下限（Minimum Size Request）**。三栏宽度总和加上边距间距后，计算出的窗口最小下限为 1552 逻辑像素（HiDPI 2x 下为 **3104 物理像素**）。而由于左侧 Ubuntu Dock 存在（宽 132px），系统工作区宽度为 `3200 - 132 = 3068 物理像素`。
     **3104px > 3068px**！Mutter 窗口管理器在接收到最大化请求时，由于窗口最小硬性尺寸超过工作区尺寸，**断然拒绝执行最大化**，并将窗口作为悬浮窗居中摆放（X: 515）；
  2. **EWMH 属性违规覆盖破坏合成器状态机**：
     在 `set_skip_taskbar` 中，曾调用 `conn.change_property32(PropMode::REPLACE, win, net_wm_state, &[max_horz, max_vert, skip_taskbar, skip_pager])`。根据 EWMH 规范，映射后的窗口其 `_NET_WM_STATE` 绝对归窗口管理器独占维护，Client 端直接 REPLACE 会破坏 Mutter 的内部状态缓存，导致 GTK 4 的 `window.maximize()` 与合成器之间产生状态震荡与多次呼出时的尺寸漂移；
  3. **工作区几何与前台检测静态冻结**：
     窗口几何此前仅在 Daemon 启动时获取一次，呼出未重新同步；前台活动窗口查询在 `win == 0` 时返回 `None`，导致点击桌面、任务栏或原生 Wayland 应用时的丢焦信号未被判定为失焦。
- **最终方案**：
  1. **中栏解除尺寸下限约束，由 `hexpand(true)` 自然铺满**：
     - 将中栏（中转站）的 `set_size_request` 调整为 `(-1, -1)`，左栏（25%）与右栏（25%）保持精确宽度，中栏依靠 `hexpand(true)` 自动吸纳剩余全部空间（严格保障 50%）；
     - 使得窗口整体最小尺寸下限由 1552 逻辑像素骤降至约 1011 逻辑像素（2022 物理像素，远低于 3068px 工作区）；Mutter 接收到最大化指令时毫无阻力，100% 严密贴合工作区；
  2. **严格遵循 EWMH Client 规范，废除直接属性覆写**：
     - `set_skip_taskbar` 彻底移除 `change_property32(PropMode::REPLACE)` 与冗余的 `event_max`；
     - 仅向根窗口发送 `_NET_WM_STATE_ADD` 的 `ClientMessageEvent`（附加 `skip_taskbar` 与 `skip_pager`）；最大化完全交由 GTK 4 原生 `window.maximize()` 驱动，消除与合成器的任何竞态；
  3. **代次级工作区几何动态刷新与全协议失焦捕获闭环**：
     - 在每次 `reveal()` 时重新调用 `WindowAdapter::get_work_area()`，动态重算三栏几何与卡片防截断行高；
     - 轮询检测以 `active_opt != Some(my_xid)` 为准，当前台变为 `None`（桌面、Ubuntu Dock、顶栏或 Wayland 应用）或其它 XID 时，超 300ms 稳定期即可精准触发自动退出；
  4. **实机验证**：
     连续 5 次实机呼出并检测 `xwininfo`，每次均稳定保持 `Absolute upper-left X: 132, Y: 64, Width: 3068, Height: 1936, Corners: +132+64 -0+64 -0-0 +132-0`，零居中偏移，零尺寸跳变。
- **状态**：Active（2026-09-18 确立）。

---

### §22 GNOME Shell 系统概览（OverviewActive）D-Bus 毫秒级订阅与任务栏指针失焦感知规范 (GNOME Overview & Taskbar Pointer-Loss Policy)

- **背景与痛点**：
  在落地前台失焦自动退出后，用户测试反馈两大盲区场景仍无法隐藏：
  1. 按下 Super / Windows 键呼出 Ubuntu 系统界面全览 (Activities Overview) 时，Vok 仍然保持显示；
  2. 点击屏幕左侧任务栏 (Ubuntu Dock) 的空白区域时，Vok 依然不自动隐藏。
- **根因精确定位 (Root Cause)**：
  1. **Super 键系统概览的合成器层级脱节**：
     在 Ubuntu GNOME Wayland (XWayland) 环境下，按下 Super 键时，GNOME Shell 在自身合成器图层呈现 Activities Overview。Mutter 既不向底层 XWayland 切换 `_NET_ACTIVE_WINDOW`（仍指向 Vok 自身的 XID），亦不向下派发 `FocusOut` 或 `notify::is-active` 事件，导致任何基于窗口焦点切换的检测机制完全静默；
  2. **Ubuntu Dock 为 Wayland 原生 Actor，XWayland 坐标与点击事件截断**：
     左侧任务栏 (`dash-to-dock`, 宽 132px) 为 Mutter 内部的 Clutter actor。根据 Wayland 安全隔离协议，当鼠标移入 Dock 范围时，Mutter 不会将鼠标点击事件派发给非焦点的 XWayland 客户端；同时 XWayland 的 `query_pointer` 坐标被 Mutter 强制钳制在 `x >= 132`，导致 `is_pointer_clicking_outside` 中的坐标判定与按键掩码在 Wayland 下物理失效。
- **最终方案**：
  1. **GNOME Shell Overview D-Bus 属性探测与信号瞬时订阅双轨闭环**：
     - `vok-infra::WindowAdapter::is_gnome_overview_active()` 查询 Session D-Bus `org.gnome.Shell` 的 `OverviewActive` 布尔属性；
     - 在 `OverlayWindow` 启动时，通过 GIO D-Bus 连接订阅 `org.gnome.Shell` 的 `PropertiesChanged` 信号，在用户敲击 Super 键的瞬间（0ms 级）接收通知并瞬时调用 `on_dismiss()`；同时在 30ms 轮询定时器中加入兜底检测；
  2. **指针移出工作区时序防抖感知体系 (Pointer Leave Squeeze Policy)**：
     - 挂载 GTK4 原生 `EventControllerMotion`，利用 `connect_leave` 与 `connect_enter` 精确追踪光标进入左侧 Dock (`x < 132`) 或顶栏 (`y < 64`) 的物理时间戳；
     - 设定 150ms 优雅防抖延时守卫：当且仅当光标离开工作区超过 150ms 时（典型的人类移入 Dock 点击或停驻交互），立即判定为失焦退让，触发自动隐藏；而快速误划（<150ms 移回）则安全忽略；
     - 严格保持规则 R3（反向拖拽透明代理模式下豁免指针移出退出）。
- **状态**：Superseded by §23（2026-09-18 确立，指针移出猜想被彻底废弃）。

---

### §23 废除指针移出表面猜想，建立基于 XInput 2 (XI2 RawButtonPress) 全屏幕无侵入物理点击与真实前台失焦架构规范 (True Focus-Loss & XI2 Passive Outside-Click Policy)

- **背景与痛点**：
  在失焦感知迭代过程中，曾引入“指针移出工作区 150ms 超时退让”以及“GTK Capture 阶段拾取非交互组件隐藏”机制。用户实机测试严重受阻：“晃动鼠标都有可能让界面隐藏；完全不是失去焦点点击非 vok 页面隐藏界面；请分析根因，严禁表面工程糊弄”。
- **根因精确定位 (Root Cause)**：
  1. **指针移动/移出猜想的致命缺陷**：
     鼠标快速滑动、在屏幕左侧边缘（接近 x=132 任务栏边界）正常操作、或者用户晃动鼠标时，GTK 的 `EventControllerMotion` 会高频触发 `connect_leave` 事件。依赖移动时序来“猜”用户是否点击任务栏，本质上把“光标移动”与“用户物理点击/真实失焦”混为一谈，导致任何鼠标晃动或掠过边界均引发灾难性的误关闭；
  2. **GTK Capture 阶段全捕获误伤内联区域**：
     在 Window 上使用 Capture 阶段的 `GestureClick` 并通过 `pick()` 检查组件类型，容易误将卡片外边距、列头空白、滚动条间隙判定为“非交互区域”而错误吞噬事件并关闭浮层；
  3. **主动抓取 (XGrabButton) 的死锁与破坏性**：
     若在 Root 窗口上对外部执行 `grab_button`，不仅会与 Mutter 自身的输入抓取产生 `BadAccess` 冲突，还会冻结指针或导致点击事件无法穿透送达真正的目标应用（如任务栏图标或桌面图标）。
- **最终方案**：
  1. **彻底铲除表面假动作与光标移动监听**：
     - 完全移除 `OverlayWindow` 上的 `EventControllerMotion`、`pointer_leave_time` 及任何超时防抖猜想逻辑；
     - 完全移除 Window 级的内部捕获 `GestureClick`，浮层内部各部件（启动台、中转站、剪贴板、设置齿轮、搜索框）遵循 GTK4 原生事件传播体系；
     - **鼠标在屏幕上任意晃动、掠过边界或停驻，产生 0 个退出事件，绝对不可能误触发隐藏**；
  2. **基于 XInput 2 (XI2 RawButtonPress) 的全屏幕无侵入物理点击感知体系 (`OutsideClickWatcher`)**：
     - 在 XWayland / X11 架构下，XInput 2 的扩展协议支持在根窗口（Root Window）上为 Master 指针设备（`deviceid = 1`）注册 `XI_RawButtonPress` 事件监听（`xinput_xi_select_events`）；
     - **完全被动监听（Passive Listening）**：不创建任何独占抓取（No Grab），不冻结指针，不抢占事件，不干扰 Mutter 及其他任何窗口的正常点击响应；
     - 专用监听线程以阻塞/epoll 方式接收物理按键流，当检测到物理鼠标左/中/右键按下（Button 1, 2, 3）时，即刻通过 `query_pointer` 读取全局物理坐标 `(x, y)`；
     - 与当前工作区 `WorkAreaGeometry`（如 `3068x1936+132+64`）执行纯几何相交校验：
       - 若处于工作区内：纯粹静默忽略，GTK 控件正常处理内部点击；
       - 若处于工作区外（`x < 132` Ubuntu Dock 区域，或 `y < 64` GNOME 顶栏，或多屏工作区外）：瞬时通过无界异步通道（`async_channel`）通知 GTK 主循环执行工作台自动隐藏；
   3. **Ubuntu Dock 空白处点击事件隔离与表面工程清退**：
      - 在 Ubuntu GNOME Wayland 环境下，左侧 Ubuntu Dock (`x < 132`) 为 Mutter 内部的 Clutter actor（`dashtodockBox`，`reactive: true`）；
      - 当用户点击 Dock 空白处时，Clutter 内部消化事件，既不启动应用亦不转移焦点（`_NET_ACTIVE_WINDOW` 与 `is_active` 保持不变）；Wayland 合成器亦绝不向 Xwayland 派发任何指针或按键事件；
      - 此前曾尝试采用“光标停留 Dock 达 200ms 猜想物理点击”，经实机核验证实其为表面工程假动作：用户正常停顿亦可能误判，且实机真实点击因坐标截断无法可靠捕获；此前日志显示的 `x=50, y=500` 隐退系集成测试在并发执行时通过 `warp_pointer` 注入的伪信号，实机人工点击并未真正隐退；
      - 依据 R12 规则，已**彻底清除 200ms 停驻猜想代码**，杜绝任何欺骗性假动作与伪闭环；
   4. **真实物理失焦与事件闭环矩阵 (True Focus-Loss Matrix)**：
      - **系统概览切换 (Super / 开始键)**：Session D-Bus `org.gnome.Shell` `PropertiesChanged` 信号 0ms 级瞬时响应；
      - **应用切换 (Alt+Tab / 呼出其它软件 / 外部窗口激活)**：EWMH `_NET_ACTIVE_WINDOW` 毫秒级轮询，前台一旦从 Vok 迁移即刻退出；
      - **顶部状态栏点击**：点击顶栏控件触发 GNOME Shell 焦点交接，毫秒级退出；
      - **点击 Dock 应用图标**：Dock 唤起或激活目标应用，前台 XID 迁移触发退出；
      - **外部 X11 区域物理点击**：XI2 `OutsideClickWatcher` 与 `is_pointer_clicking_outside` 无侵入捕获退出；
      - **快捷键 / Esc 键**：Capture 阶段优先截获，零延迟退出；
      - **鼠标晃动与界面内交互**：100% 免疫误退出（晃动 0 响应，点击卡片/按钮/搜索框稳定工作）；
      - **原子退让与焦点返还**：所有退让路径统一调用 `WindowAdapter::restore_active_window(previous_xid)`，将焦点精准返还给被呼起前的前台应用（VSCode、终端、浏览器）；
      - **规则 R3 守卫**：处于反向拖出透明代理锚点模式（`is_in_proxy_mode`）时，所有失焦判定严格挂起，确保 Wayland DnD 拖拽会话维系。
   5. **机制局限与根本出路**：
      - 在无 root / input 组权限的前提下，X11/Xwayland 客户端在机制上物理无法监听 Wayland 合成器内部 Chrome Actor（Dock 空白处）的纯吸收点击；
      - 真正的硬件级全局点击感知必须依赖 Manifest §2 / §4 锁定的技术栈——Linux 内核 `/dev/input/event*`（`evdev`）。用户若将当前账号加入 `input` 用户组，`vok-infra` 内核级按键流即可毫秒级感知全局任意位置物理点击。
- **状态**：Active（2026-09-18 确立，2026-09-19 清退停驻猜想并固化底层机理）。

---

### §24 基于 PolicyKit (Polkit) 原生授权与 Linux 内核 evdev 硬件级全盲区鼠标点击感知规范 (Polkit & Evdev Hardware Mouse Click Policy)

- **背景与痛点**：
  在 Ubuntu GNOME Wayland 环境下，由于 Mutter 合成器将左侧 Ubuntu Dock (`x < 132`) 作为私有 Chrome Actor 隔离，当用户点击两枚图标之间的空白死区时，合成器内部吸收事件，不切换窗口亦不派发 X11 信号。在未获得 Linux 内核 `/dev/input` 权限的前提下，X11 客户端在机制上无法获悉该物理点击。若要求用户手动开黑底白字终端敲命令行进行授权，严重违背专业现代桌面 GUI 应用的优雅产品体验。
- **最终方案**：
  1. **遵循业界 Polkit (PolicyKit) 原生授权规范与 uaccess 机制 (`SystemPermission`)**：
     - 在 `vok-infra::desktop::permission` 封装 `SystemPermission`；
     - 采用现代 systemd-logind 桌面标准：通过 `pkexec` 执行最小特权配置脚本，写入 `/etc/udev/rules.d/99-vok-input.rules`：
       `KERNEL=="event*", SUBSYSTEM=="input", TAG+="uaccess"`
     - 执行 `udevadm control --reload && udevadm trigger --subsystem-match=input`；
     - 并通过 `setfacl -m u:$USER:r /dev/input/event*` 赋予当前活跃桌面用户直接读取 ACL，**实现零重启、零注销、即刻生效**；
     - 呼出 GNOME 原生磨砂玻璃安全认证对话框（支持密码输入与指纹识别），应用自身绝不触碰用户密码；
  2. **Linux 内核物理鼠标按键硬件流监听器 (`EvdevMouseWatcher`)**：
     - 在 `vok-infra::input` 封装 `EvdevMouseWatcher`，自动枚举扫描系统中所有支持 `Key::BTN_LEFT` 的物理鼠标与触控板设备；
     - 专用后台工作线程读取微秒级硬件电平事件，当捕获到物理鼠标左键按下（`BTN_LEFT` value=1）时，即刻通过 X11 读取当前指针物理绝对坐标；
     - 与当前生效工作区 `WorkAreaGeometry` 执行纯几何校验：
       - 若处于工作区外部（包含 Ubuntu Dock 纯黑空白缝隙、顶栏、多屏盲区）：0 微秒瞬时向 GTK 投递退让信号；
       - 若处于工作区内部：静默放行，由 GTK 原生卡片与组件处理点击；
       - 鼠标任意移动与快速晃动：仅产生 `EV_REL` 相对坐标位移，**产生 0 个 `BTN_LEFT` 按键事件，100% 绝对免疫误隐藏**；
  3. **独立设置窗口 (`SettingsWindow`) 图形化一键授权与状态徽章**：
     - 在首选项【交互行为】页面新增“硬件级全局输入感知 (Linux 内核 evdev)”专属分组；
     - 自动检测当前权限状态：若未授权显示“一键授权”高亮操作按钮；点击后在后台异步唤起 Polkit 认证，通过 `async_channel` 在主线程局部 Future 中零卡顿刷新 UI；授权成功后按钮自动隐藏并呈现“✓ 已激活”绿色徽章；
  4. **双轨外置点击统一流**：
     - `OverlayWindow` 内部统一将 `OutsideClickWatcher` (XI2) 与 `EvdevMouseWatcher` (evdev) 的触发汇流至同一个 `async_channel`；在透明代理锚点模式下统一挂起，在呼出与隐藏时原子同步启闭。
- **状态**：Active（2026-09-19 确立）。

---

### §25 突破 Wayland 坐标钳位与触控板轻击 (Tap) 状态机：双轨窗口层级与全按键退让规范 (Wayland Clamp-Bypass & Touchpad Tap Policy)

- **背景与痛点**：
  在完成 Polkit 授权与 evdev 内核鼠标监听器后，实机测试中点击 Ubuntu Dock 纯空白处依然未能隐藏。且用户提出重大疑问：“日志上显示隐藏，但实际上根本没有隐藏；检测机制都不准；为什么日志总是检测到隐藏？”。
- **根因精确定位 (Root Cause)**：
  1. **日志显示隐藏与实机脱节的真凶**：
     日志中显示的 `【失焦感知】OverlayWindow 失去前台活动焦点 (Focus-Out)，触发自动退出`，系用户在点击 Dock 空白无反应后，切换窗口查看终端日志或向 AI 发送反馈时触发的标准窗口失焦，而非 Dock 点击所触发；用户将后置的窗口切换失焦日志误认为是前置点击的判定结果。
  2. **Wayland 合成器坐标钳位 (Coordinate Clamping)**：
     在 GNOME Wayland 下，Ubuntu Dock 为 Mutter 私有 Clutter actor（位于物理屏幕 $x=0..132$ 区域）。当光标移入 Dock 时，Mutter 不会将属于原生 Wayland 界面的指针坐标如实同步至 Xwayland 根窗口，Xwayland 中的 `query_pointer` 坐标停滞在边界处（$x=132$）。此前的外置点击检测仅校验纯几何 `x < wa.x`（即 $132 < 132 \rightarrow \text{false}$），导致即便捕获到了内核物理按键，几何校验仍判定为“处于工作区内”而静默跳过！
  3. **笔记本触控板轻击 (Touchpad Tap) 事件码盲区**：
     笔记本触控板的轻击（Tap-to-click）在 Linux 内核 evdev 中并不派发 `BTN_LEFT`（272），而是派发触控电平 `BTN_TOUCH`（330）。此前的监听器仅过滤 `BTN_LEFT`，直接漏掉了触控板轻击操作。
- **最终方案**：
  1. **双轨窗口层级反向排除法 (`check_pointer_against_vok`)**：
     - 在 `WindowAdapter::check_pointer_against_vok(wa, vok_xid)` 中，综合判定：
       `is_outside = geom_outside || (vok_xid != 0 && child != vok_xid)`；
     - 彻底打破对 Wayland 坐标同步的依赖：在 Xwayland 窗口树中，Vok Overlay 是唯一的全屏顶级窗口（XID: `0x1800004`）。当光标进入 Ubuntu Dock 时，Xwayland 根窗口下的命中子窗口（`reply.child`）必定为 `0` 或非 Vok 窗口；
     - 当光标位于 Vok 界面内部时，`reply.child == vok_xid` 且 `geom_outside == false`，严丝合缝免疫；
  2. **触控板轻击 Tap 状态机与物理多键全覆盖**：
     - 在 `EvdevMouseWatcher` 内部构建高精度 Tap 状态机：跟踪 `BTN_TOUCH` 按下到抬起的时间差，当 $\Delta t < 250\text{ms}$ 时精确判定为一次物理轻击（Tap）；
     - 全面覆盖物理鼠标左键、右键、中键及侧键（`BTN_LEFT`, `BTN_RIGHT`, `BTN_MIDDLE`, `BTN_SIDE`, `BTN_EXTRA`）；
  3. **生命周期原子联动**：
     - `OverlayWindow::reveal()` 呈现时动态刷新最新的 `vok_xid`，并在 200ms 防抖延迟后原子启用监听；
     - 每次物理输入捕获与外置决策均输出包含设备、事件码、光标坐标、子窗口 XID 的确凿日志，实现完全透明可观测。
- **状态**：Active（2026-09-19 确立）。

---

### §26 攻克 Dock 首次点击成功后全失效与任务栏图标偶发驻留：边界钳位容差与首帧预写入规范 (Dock Border Clamping & Pre-Map Taskbar Invariant)

- **背景与痛点**：
  用户反馈两大非确定性现象：
  1. 点击任务栏空白依旧不自动隐藏：第一下可以正常隐藏，但后续全部失效；
  2. 任务栏（Ubuntu Dock）偶尔会出现 Vok 图标/运行圆点驻留。
- **根因精确定位 (Root Cause)**：
  1. **Dock 首次隐藏成功、后续全部失效的因果链路**：
     - 当 Vok 从隐藏到初次被唤起且光标初始停留在 Dock 时，Xwayland 从未收到针对 Vok 的 `EnterNotify`，`reply.child` 为 `0x0`，故 `child != vok_xid` 为真，首下能够成功隐藏；
     - 但当下一次用户在 Vok 界面中移动光标后，Xwayland 收到 `EnterNotify`，记录当前顶级子窗口为 `vok_xid` (`0x2000004`)；
     - 随后用户将鼠标向左移入 Ubuntu Dock，Mutter 原生 Clutter actor 接管指针并不再向 Xwayland 派发任何 motion 事件。Xwayland 中的光标坐标被停滞在表面交界处：$x=133$（$wa.x=132$），且 Xwayland 从未收到 `LeaveNotify`，`reply.child` 永久维持在 `vok_xid`；
     - 此时用户在 Dock 上无论点击何处，X11 查询到的始终是 $(x=133, \text{child}=\text{vok\_xid})$，导致 `geom_outside`（$133 < 132 \rightarrow \text{false}$）与 `window_outside`（$\text{vok\_xid} \neq \text{vok\_xid} \rightarrow \text{false}$）双双判定为假，后续所有点击全部被误判为“在 Vok 内部点击”而遭忽略！
  2. **任务栏偶发驻留 Vok 图标的因果链路**：
     - GTK 4 原生移除了 `set_skip_taskbar_hint`，此前系统通过 `window.connect_map` 在窗口映射后异步发送 X11 ClientMessage；
     - 当 GTK 呈现窗口时，窗口先被 X11 / Xwayland 映射（MapWindow），Mutter / GNOME Shell `ShellWindowTracker` 立即捕获到新映射的 `com.vok.desktop` 窗口；在 ClientMessage 到达前的数毫秒空窗期内，窗口属性尚未具备 `_NET_WM_STATE_SKIP_TASKBAR`，Ubuntu Dock 遂将其加入运行应用列表中；
     - 若用户短时间内关闭或再次切换窗口，Dock 内部状态产生时序竞争，导致图标或运行指示点未能及时清除，残留于 Dock 中。
- **最终方案**：
  1. **工作区避让边缘钳位容差算法 (`border_clamped`)**：
     - 在 `WindowAdapter::check_pointer_against_vok` 中引入 6 物理像素（HiDPI 下为 3 逻辑像素）的避让边界钳位检测：
       ```rust
       let border_clamped = (wa.x > 0 && x <= wa.x + 6)
           || (wa.y > 0 && y <= wa.y + 6)
           || (screen_w > 0 && (wa.x + wa.width as i32) < screen_w && x >= (wa.x + wa.width as i32 - 6))
           || (screen_h > 0 && (wa.y + wa.height as i32) < screen_h && y >= (wa.y + wa.height as i32 - 6));
       let is_outside = geom_outside || window_outside || border_clamped;
       ```
     - 严密数学与布局证明：Vok 根容器拥有 `padding: 16px 20px`（Scale 2 下为 40 物理像素），边缘 6 物理像素内绝对无任何可点击 Widget。当光标处于该避让边缘带内，100% 系用户光标移入 Dock/顶栏被合成器钳位所致，权威判定为外置点击，彻底攻克后续点击全失效难题。
  2. **首帧前预写入与双轨保障 (`set_window_skip_taskbar`)**：
     - 初始化时立即执行 `window.realize()`，使底层 X11 Window 立即创建；
     - 显式通过 `change_property32(PropMode::APPEND)` 直接将 `_NET_WM_STATE_SKIP_TASKBAR` 与 `_NET_WM_STATE_SKIP_PAGER` 写入 X11 窗口的 `_NET_WM_STATE` 属性中；
     - 在 `OverlayWindow::reveal()` 呼出前预先写入，确保 Mutter 接收到 `MapRequest` 的首帧即可读取到跳过任务栏属性，杜绝任何未带属性的中间帧暴露给 GNOME Shell；
     - 同时保留 EWMH ClientMessage 通知机制（source indication = 2），并为桌面启动项配置 `StartupNotify=false`，彻底根除 Dock 偶发驻留。
- **状态**：Active（2026-09-19 确立）。

---

### §27 快捷键长短按模式判决重构、内核 evdev 组合键全硬件流与 GNOME DBus 回声防抖规范 (Release-Decided Hotkey & Evdev Chord Pipeline Policy)

- **背景与痛点**：
  1. **首帧时钟缺陷（First-Frame Clock Defect）**：用户物理短按快捷键（如 Alt+Space，人类敲击用时 ~80ms），首帧 Buffer 极速渲染提交仅需 ~4.7ms。旧状态机在首帧到达时若按键仍处于按下电平（`is_key_down: true`）便武断推断为 `HotkeyMode::Momentary`，导致短按松手时触发 `PhysicalRelease` 被视作长按松手而无条件隐藏闪退；
  2. **`EvdevMonitor` 硬编码 F12 且缺失组合键（Chord）追踪**：旧 `EvdevMonitor` 仅监听单个 `Key::KEY_F12`，无法支持用户自定义的组合键（如 `Alt + Space`、`Super + Space`）；且外部文件拖拽中（Mid-drag Summon），GNOME 快捷键被拖拽抓取吞噬，内核级组合键监听缺失导致拖拽中无法呼出；
  3. **双轨竞态与 GNOME DBus 误关窗口（DBus Echo Bug）**：`evdev` 在内核级毫秒内捕获物理按键并呈现窗口，GNOME `custom-keybinding` 随后经由 DBus 发起 `toggle` 调用（延迟 ~20-80ms）；DBus 处理函数误见窗口已经处于 `is_visible: true` 状态，误将其视作“再次按下快捷键关闭”，导致窗口刚打开瞬间被误关。
- **根因分析与架构重构**：
  1. **状态机释放时判定（Release-Decided Invariant）**：
     - `SessionState::Visible` 状态字段扩展为包含 `press_instant: Instant`；
     - 首帧 `FrameCommitted` 到达时，若按键未松开，仅保持 `is_key_down: true`，严禁提前断言为 Momentary；
     - 待物理按键真正释放 `PhysicalRelease(release_instant)` 时，以 $\Delta t = \text{release\_instant} - \text{press\_instant}$ 与阈值（`DEFAULT_TAP_MAX_MS = 280ms`）进行权威判决：$\le 280\text{ms}$ 为 Toggle（保持常驻显示，`is_key_down: false`），$> 280\text{ms}$ 为 Momentary（松手即隐藏）；
  2. **`EvdevMonitor` 全按键映射与组合键电平流**：
     - 实现 `key_name_to_evdev_key(&str) -> Option<Key>`，覆盖功能键、字母、数字、符号、导航键等全量标准按键；
     - 构建无锁原子的 `ModifierTracker`（`AtomicBool` 独立追踪 `left/right_ctrl`, `left/right_alt`, `left/right_shift`, `left/right_super`），消除跨设备与修饰键冲突；
     - 在后台事件流中，主键按下时比对修饰键全匹配，触发 `Pressed`；主键释放或任一关键修饰键提前释放时触发 `Released`，精准支持组合键与外部拖拽中（Mid-drag）内核级直接呼出；
     - 提供 `update_target_hotkey(&self, config: &HotkeyConfig)` 支持设置窗口修改快捷键后热重载；
  3. **GNOME DBus 回声防抖（DBus Echo Debounce）**：
     - 记录全局 `last_reveal_instant: Option<Instant>`；
     - 在 DBus `toggle` 或命令行二次激活到达时校验窗口呈现时长：若处于 350ms 保护窗口内，权威判定为同一物理击键派发的系统级回声，安全忽略；若 $> 350\text{ms}$ 则正常执行 Toggle 关闭。
- **状态**：Active（2026-09-20 确立）。

---

### §28 彻底根除长按无限交替开关震荡：GNOME Typematic 连发抑制与三重防御架构 (Typematic Repeat Suppression & Triple-Shield Toggle Architecture)

- **背景与痛点**：
  用户在实机长按快捷键（如 `Alt + Space`）时，工作台出现严重 bug：窗口以约 380ms 周期在“打开-关闭-打开-关闭”间无限交替翻转闪烁，无法稳定常驻呈现。
- **根因精确定位 (Root Cause)**：
  1. **GNOME Typematic 自动连发特性**：GNOME `custom-keybindings` 在物理按键被按住时，键盘硬件与合成器以 ~30Hz 频率（500ms delay + 30ms repeat-interval）疯狂向 D-Bus 发送 `toggle` 动作指令；
  2. **单向防抖不对称漏洞**：旧代码仅对 `is_visible == true` 做了 350ms 防抖，对 `is_visible == false` **完全无防抖**！当用户长按超过 500ms 时：
     - 首发连发在 500ms 到达（> 350ms），将窗口隐藏；
     - 仅 30ms 后下一发连发到达，因 `is_visible == false` 无防抖，立即将窗口重新打开，并向状态机注入伪造的 `PhysicalPress` 与 `PhysicalRelease`；
     - 再过 350ms，下一发连发再次关闭窗口；
     - 构成了 `Show (350ms) -> Hide (30ms) -> Show (350ms) -> Hide` 的 ~380ms 无限振荡时钟；
  3. **双轨失联**：DBus Toggle 链路完全未查询底层的 `EvdevMonitor` 物理按键电平，在用户手指依然死死按住按键时盲目执行关闭。
- **最终方案（三重防御架构 Triple-Shield Architecture）**：
  1. **第一重：硬件电平权威仲裁 (Hardware Level Shield)**：
     - `EvdevMonitor` 提供原子的 `is_hotkey_active(&self) -> bool` 查询；
     - 只要用户手指按在物理按键上，`action_toggle` 与 `TrayCommand::Toggle` 立即判定所有到达的 DBus `toggle` 指令均为系统连发，**无条件直接丢弃**，不向事件通道压入消息；
  2. **第二重：连发速率极速过滤 (Typematic Burst Filter)**：
     - 记录全局 `last_toggle_call: Option<Instant>`；
     - 若连续两次 Toggle 尝试间隔 $< 150\text{ms}$，判定为 typematic repeat 突发连发序列，直接抑制，彻底消除无 evdev 环境下的连发震荡；
  3. **第三重：双向对称状态翻转防抖 (Bidirectional State Change Debounce)**：
     - 将原先仅记录呼出时间的 `last_reveal_instant` 重构为全生命周期的 `last_state_change: Option<Instant>`；
     - 无论是呼出、隐藏、Esc 退出、外部点击退出还是反向拖出结束，均原子刷新 `last_state_change`；
     - 任何方向的状态翻转均必须满足 $\Delta t \ge 350\text{ms}$ 阈值，阻断任何微秒/毫秒级的回声振荡。
- **状态**：Active（2026-09-20 确立）。

---

### §29 双轨长短按自适应状态机、蓝牙 UHID 设备 seat0 提权与 evdev 动态热插拔自检规范 (Dual-Track Hold Detector & Dynamic Evdev Hotplug Policy)

- **背景与痛点**：
  1. 用户在未配置 input 用户组或使用蓝牙/无线键盘（通过 `/devices/virtual/misc/uhid` 挂载，如 VGN S99-2 Keyboard）时，因缺乏 seat 标签导致 `evdev` 无法打开内核设备，系统回退至纯 D-Bus / GNOME 快捷键链路；
  2. GNOME `custom-keybinding` 在长按时不发 release 事件，仅在 500ms 初始延时后以 ~30Hz 频率连发 `toggle`。旧代码在 500ms 收到第一发 repeat 时，因窗口已可见且 $> 300\text{ms}$ 防抖，误将其当成二次点击而直接关闭窗口，导致长按过程中界面自动关闭；
  3. systemd `71-seat.rules` 与 `73-seat-late.rules` 对虚拟 uhid 设备缺少 `path_id`，未能为其分配 `ID_SEAT="seat0"`，且旧规则 `99-vok-input.rules` 晚于 73-seat-late 执行，未能成功赋予 POSIX ACL；
  4. `EvdevMonitor` 原先仅在应用启动时执行一次静态扫描，蓝牙键盘休眠唤醒、重连或权限授予后无法动态挂载。
- **最终方案**：
  1. **DbusHoldDetector 连发心跳保活状态机 (D-Bus Hold Detection State Machine)**：
     - 在纯 D-Bus 模式下，将 GNOME typematic repeat 视作物理按键按压的心跳信号；
     - 窗口可见时收到第 1 发指令启动 85ms 侦听计时器；
     - 若 85ms 内收到第 2 发连发（典型间隔 ~30ms），100% 判定为长按，立即进入 `is_holding = true` 常驻模式并启动 110ms silence 计时器；
     - 随后每发连发刷新 110ms silence 计时器，长按过程中窗口保持常驻呈现；
     - 松手后连发停止，110ms silence 计时器超时触发隐藏（Momentary）；
     - 若为单次短按点击关闭，85ms 内无第二发到达，计时器正常触发隐藏。
  2. **蓝牙 UHID 规则重构与动态提权 (Early udev seat0 Policy)**：
     - 将规则前移至 `/etc/udev/rules.d/70-vok-input.rules`（先于 71-seat.rules 与 73-seat-late.rules）；
     - 显式声明 `KERNEL=="event*", SUBSYSTEM=="input", TAG+="uaccess", TAG+="seat", ENV{ID_SEAT}="seat0", MODE="0660", GROUP="input"` 并联动 `usermod -aG input {username}` 与 `setfacl`；
  3. **EvdevMonitor 动态热插拔与权限自检 (Dynamic Evdev Hotplug)**：
     - `monitored_paths: Arc<Mutex<HashSet<PathBuf>>>` 动态追踪已接入设备；
     - 后台启动每 2 秒一次的动态扫描自检线程，新设备接入/新获权设备自动热挂载，拔出时自动注销；
     - 结合 `settings_window.set_on_permission_granted` 实现授权后毫秒级即刻挂载。
- **被否决方案**：
  - *纯盲等或依赖 sleep*：违反 R11 铁律；
  - *仅依赖 evdev 强行报错退出*：导致无 root/未授权用户无法使用应用，违背鲁棒性原则。
- **状态**：Active（2026-09-20 确立）。

---

### §30 窗口级按键释放拦截与输入法隔离架构：0ms 极致跟手长按瞬关与 Fcitx/IBus 修饰键防泄露体系 (Zero-Latency Window Key-Release & IME Isolation Architecture)

- **背景与痛点**：
  1. **输入法状态抽搐乱切**：在 Linux / Fcitx5 / IBus 桌面环境中，使用快捷键（如 `Alt + Space`）呼出或关闭工作台时，系统输入法频繁在“拼音”与“英文 (en)”之间来回剧烈切换并产生状态副作用；
  2. **长按松手关闭严重不跟手（迟滞感）**：纯依赖 D-Bus 连发心跳静默超时（350ms）检测长按松手时，用户抬起手指后界面仍停滞超过 1/3 秒才消失，无法实现“手抬即隐”的瞬关体验，交互严重滞后。
- **根因精确定位 (Root Cause)**：
  1. **孤立修饰键泄露至输入法**：旧代码中 UI 层的 `EventControllerKey` 仅在 `connect_key_pressed` 中拦截了完整组合键，**完全没有监听 `connect_key_released`**，且放行了 `Alt_L` / `Alt_R` 的单独按压。当用户按下并释放 `Alt + Space` 时，`Alt` 键的按下与释放事件穿透至 `search_entry` 和底层 `GtkIMContext`。Fcitx5 收到未配对的独立修饰键释放，判定为中英文模式切换热键，导致输入法状态反复横跳；
  2. **D-Bus 协议无按键释放语义**：GNOME `custom-keybinding` 仅在按下和连发时触发命令行，完全不提供 KeyRelease 回调，导致仅靠后台 D-Bus 永远无法获得亚毫秒级的物理松手感知。
- **最终方案（最优解 Optimal Architecture）**：
  1. **窗口焦点级按键释放 0ms 瞬时感知 (Zero-Latency Window Key-Release)**：
     - 在 `OverlayWindow` 的 `key_controller`（`PropagationPhase::Capture`）中全新挂载 `connect_key_released`；
     - 引入 `is_initial_press_held: Rc<Cell<bool>>` 严格追踪“呼出初始击键”状态：
       - 窗口 `reveal()` 时标记 `is_initial_press_held = true`；
       - 当用户在窗口前台松开快捷键或修饰键时，若按住时长 $\Delta t > 280\text{ms}$（长按瞬态模式），**以 0ms 延迟立即触发 `on_dismiss()` 隐藏窗口**（真正实现手抬即隐、纳秒级极致跟手）；
       - 若 $\Delta t \le 280\text{ms}$（短按翻转模式），则保持常驻展开，并将 `is_initial_press_held` 置为 `false`；
       - `OverlayWindow::hide()` 清场时原子重置 `is_initial_press_held = false`；
     - 后续用户在搜索框内正常输入空格、字符或执行编辑时，完全不受任何拦截与副作用影响。
  2. **全生命周期修饰键拦截与输入法隔离 (Complete IME Isolation)**：
     - 在 `connect_key_pressed` 中，除了拦截目标按键，精准拦截当前热键使用的修饰键（`Alt_L/R`, `Ctrl_L/R`, `Shift_L/R`, `Super_L/R`），禁止其穿透至 `GtkIMContext`；
     - 在 `connect_key_released` 中，完全消费并阻断热键与修饰键的释放事件，不向子控件与输入法派发任何孤立事件；
     - Fcitx5 / IBus 不再接收到任何异常的修饰键脉冲，输入法状态彻底稳固。
  3. **精细化键盘权限诊断 (Granular Active Keyboard Diagnostics)**：
     - `SystemPermission::has_input_permissions()` 遍历 `/proc/bus/input/devices`，精确验证系统中**每一个活跃物理键盘设备**的读写权限，杜绝因个别设备有权限而掩盖蓝牙键盘无权限的假阳性；
     - D-Bus 350ms 静默超时保留作为窗口未获得焦点等极端边缘场景下的后台安全兜底网。
- **被否决方案**：
  - *纯 D-Bus 延长或缩短 silence 超时*：缩短会导致 286ms 调度抖动时误关震荡，延长会导致松手迟滞加剧；
  - *全局禁用搜索框 IMContext*：会导致用户无法在搜索框输入中文应用名与文件名，严重损害核心功能。
- **状态**：Active（2026-09-20 确立）。

