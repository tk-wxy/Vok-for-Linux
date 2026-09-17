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

