# Vok (Linux) 目标技术架构规范

> 状态：2026-09-10 确立。本文档定义 Linux 原生 Vok 项目的核心技术架构、线程模型、状态机与系统级协议实现规范。

---

## 1. 设计哲学与优先级

架构取舍严格遵循以下优先级：

1. **正确性与安全性**：文件拖放、输入拦截、数据持久化 100% 正确；严禁擅自修改或删除用户原文件；
2. **确定性与可预测性**：呼出、拖入、拖出、隐藏和前台焦点交接可预测，严禁通过 `sleep` 猜测时序；
3. **资源有界性**：长期后台常驻内存严格控制在 **< 40MB**；存储与缩略图缓存具有明确的容量与清理上限；
4. **模块隔离与可测试性**：核心领域模型与状态机纯净独立，基础设施层可 Mock、可单测；
5. **极限性能**：在上述条件满足的前提下，实现瞬时呼出（< 16ms）与轻盈的原生渲染。

---

## 2. 总体分层结构

项目采用现代 Rust 模块化工作区（Workspace）分层架构，严格禁止反向依赖：

```text
┌─────────────────────────────────────────────────────────────┐
│                          vok-app                            │
│   (GTK 4 / Libadwaita 表现层、UI 组合根、CSS 主题、组件渲染)  │
└──────────────────────────────┬──────────────────────────────┘
                               │ 依赖
┌──────────────────────────────▼──────────────────────────────┐
│                          vok-core                           │
│   (纯领域层：工作台会话状态机、GenerationId、中转站引用模型)    │
│   * 零平台依赖，零 GUI 依赖，纯 Rust 逻辑，100% 可单测        │
└──────────────────────────────┬──────────────────────────────┘
                               │ 依赖
┌──────────────────────────────▼──────────────────────────────┐
│                          vok-infra                          │
│   (Linux 基础设施适配层：封装全部系统调用与外部协议)           │
│   ├─ input: evdev (物理键态监听) + uinput (虚拟按键注入)       │
│   ├─ window: gtk4-layer-shell (Wayland) + x11rb (X11 适配)  │
│   ├─ storage: rusqlite (WAL 单写者) + 内容寻址外置资产         │
│   └─ desktop: XDG Desktop Entry + DBus (zbus 文件管理器定位) │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. 进程、线程与资源所有权模型

Vok 采用**单一进程 + 显式资源所有权多线程**模型，杜绝多进程 IPC 膨胀与无界并发：

| 线程角色 | 拥有资源 / 职责 | 与其他线程交互方式 |
| :--- | :--- | :--- |
| **GTK UI 主线程 (Main Loop)** | `GtkApplication`、Window、Widget 树、RenderNode 渲染管线、拖放 Target/Source | 接收不可变 Channel 消息；通过 `glib::idle_add_local` 排队调度 |
| **Input Monitor 线程** | 独占打开 `/dev/input/event*` 文件描述符，阻塞读取硬件按键中断 | 向 UI 线程无阻塞异步发送 `HotkeyEvent(GenerationId, KeyState)` |
| **Storage Worker 线程** | 独占 SQLite 写入连接，管理 WAL 事务、Schema 迁移与元数据写入 | 接收有界 MPSC 命令队列；返回只读数据结构 |
| **Asset / Thumbnail Worker** | 独占后台图像缩放库与 SHA-256 计算，生成 160px 缩略图落盘 | 工作完成向 UI 线程发送 `ThumbnailReady(ItemId, Path)` |

**并发禁令**：
- 禁止后台线程直接调用任何 GTK UI 对象的接口；
- 禁止 UI 线程执行任何阻塞性磁盘 I/O 或数据库同步写事务；
- 禁止散落式的无界 `std::thread::spawn`；后台任务统一提交至受控有界 Worker。

---

## 4. 工作台会话与全屏状态机 (Session Lifecycle)

每次呼出由系统输入触发，并创建唯一的代次令牌 `GenerationId`：

```text
        [物理按键按下]
              │
              ▼
           Hidden ───────────────┐
              │                  │
        (创建新 GenerationId)     │
              ▼                  │
          Revealing              │ (取消/超时)
              │                  │
         (首帧 Buffer 提交)       │
              ▼                  │
           Visible ◄─────────────┘
          /       \
[开始拖动中转项]    [松开物理按键 / 按 Esc / 失焦]
        /           \
       ▼             ▼
   Dragging        Hiding
       │             │
(完成投递/取消)   (清理 Topmost/清场)
       │             │
       └──────┬──────┘
              ▼
            Hidden
```

### 4.1 长按与短按状态机设计 (Momentary vs Toggle)
- **阈值约定**：`TAP_MAX_MS = 250ms`；
- **按下沿 (Press Event)**：
  - 生成新 `GenerationId`，记录物理按下时间戳 `t_down`；
  - 无论长按短按，立即启动 `Revealing -> Visible` 流程（确保瞬时响应）；
- **释放沿 (Release Event)**：
  - 计算按住时长 `duration = t_up - t_down`；
  - **若 `duration > TAP_MAX_MS`（长按 Momentary 模式）**：松开瞬间立即触发 `Hiding`。此模式完美契合**“抓着文件拖入中转站”**的场景：抓文件 -> 随手按下热键呼出 -> 丢进屏幕 -> 松开热键工作台瞬间消失；
  - **若 `duration <= TAP_MAX_MS`（短按 Toggle 模式）**：若呼出前为隐藏，则松开后保持显示；若呼出前已显示，则触发隐藏。

### 4.2 全屏面板呈现协议
- **Wayland (优先路线)**：
  - 使用 `gtk4-layer-shell`，设定：
    ```rust
    gtk4_layer_shell::init_for_window(&window);
    gtk4_layer_shell::set_layer(&window, gtk4_layer_shell::Layer::Overlay);
    gtk4_layer_shell::set_anchor(&window, Edge::Top, true);
    gtk4_layer_shell::set_anchor(&window, Edge::Bottom, true);
    gtk4_layer_shell::set_anchor(&window, Edge::Left, true);
    gtk4_layer_shell::set_anchor(&window, Edge::Right, true);
    gtk4_layer_shell::set_exclusive_zone(&window, 0); // 不挤占已有工作区
    gtk4_layer_shell::set_keyboard_mode(&window, KeyboardMode::OnDemand);
    ```
  - Wayland 的 Surface Commit 天生具备原子性，窗口在第一帧完全就绪后由 Compositor 在下一次刷新垂直同步呈现，从原理上消除闪烁。
- **X11 (回退路线)**：
  - 设置无边框与 `_NET_WM_WINDOW_TYPE_DOCK` / `_NET_WM_STATE_ABOVE`。

### 4.3 任务栏避让与屏幕工作区自适应
- **设计原则**：面板全屏覆盖桌面，但**严格留出系统任务栏**，方便用户随时观察系统状态或点击任务栏退出。
- **Wayland 实现**：
  - 任务栏（Waybar, GNOME Shell, KDE Panel）若声明了 `exclusive_zone > 0`，Layer Shell 浮层可通过配置边距（Margin）或挂载于 `Layer::Top` 自动根据 Compositor 分配的可用工作区缩进；
  - 若处于 `Layer::Overlay`，通过读取 `gdk::Display::monitors()` 获取各监视器有效几何区，动态设置 `set_margin(&window, Edge::Bottom, taskbar_height)`。
- **X11 实现**：
  - 读取根窗口 `_NET_WORKAREA` 属性，将窗口几何尺寸限制在工作区矩形范围内。

### 4.4 焦点暂存、捕获与原前台精准返还机制
- **呼出即入焦**：
  - 面板揭示瞬间，立即捕获键盘与鼠标焦点，默认自动聚焦于顶部全局搜索框，保证用户可直接敲击键盘输入搜索或使用方向键导航；
  - 暂存上下文：呼出前瞬间记录系统当前活动窗口句柄（X11 `_NET_ACTIVE_WINDOW` 或 Wayland Compositor 活动节点）。
- **失焦与多途径关闭**：
  - 支持快捷键再次触发（Toggle 模式）；
  - 支持按下 `Esc` 键直接退出；
  - **失焦自动关闭 (Focus-Out Auto-Close)**：当用户鼠标点击任务栏或面板外的任何其他屏幕区域时，面板感知到失焦立即触发平滑隐藏。
- **原前台精准返还**：
  - 面板 Unmap 关闭时，原子交还前台激活权给呼出前记录的原始窗口及其原焦点控件（如浏览器搜索框、IDE 编辑光标、终端）；
  - 为接下来的“直接粘贴”或后续键盘操作提供无缝衔接。

### 4.5 主界面 1:2:1 三栏架构与顶部联动搜索
主界面采用经典高效的三栏水平布局，宽度比例分配为 **1 : 2 : 1**：
```text
┌─────────────────────────────────────────────────────────────┐
│              [ 顶部全局统一搜索框 (Ctrl+K) ]            ⚙ 齿轮 │
├──────────────────┬───────────────────────┬──────────────────┤
│    启动台 (25%)   │      中转站 (50%)     │   剪贴板 (25%)   │
│  (快捷启动/文件)  │   (临时文件流动货架)   │  (历史剪贴板/消费)│
│                  │                       │                  │
│  [卡片] [卡片]   │  [卡片]       [卡片]  │  [条目]          │
│  [卡片] [卡片]   │  [卡片]       [卡片]  │  [条目]          │
│  (一行4项/整除高)│  (双列网格/整除高)    │  (单列虚拟列表)  │
└──────────────────┴───────────────────────┴──────────────────┘
```
- **顶部搜索框**：常驻顶部，单个输入词同时联动过滤左侧启动项、中间中转项与右侧剪贴板项；右上角提供独立【设置】小齿轮入口。
- **启动台 (25%)**：固定一行 4 项的小图标卡片，用于高频应用与文件一键启动；
- **中转站 (50%)**：核心视觉中枢，双列大卡片，承载临时文件拖入、暂存、拖出与消费；
- **剪贴板 (25%)**：单列密集列表，记录文本、图片与文件历史，支持一键点击直接粘贴。

### 4.6 卡片高度防截断对齐动态算法 (No-Clipping Height Algorithm)
- **痛点**：固定卡片高度在多变分辨率或缩放比下，常导致底部最后一行卡片显示被截断一半，视觉残缺；
- **核心算法**：
  启动台与中转站共享统一的高度算法。设当前功能区可用净高度为 $H_{avail}$，外边距为 $P$，卡片行间距为 $G$，理想行数为 $N$：
  $$H_{row} = \left\lfloor \frac{H_{avail} - 2P - (N - 1) \times G}{N} \right\rfloor$$
  在窗口尺寸变动或初次渲染时，GTK 布局容器动态计算并锁定 $H_{row}$，强制单行高度为计算结果，**确保默认视口中展示的最后一行卡片 100% 完整露出，绝对不被底部 UI 边框截断**。启动台与中转站基准水平线严格对称对齐。

### 4.7 独立设置窗口与增强搜索副界面架构
1. **独立设置窗口 (Settings Dialog)**：
   - 独立于全屏 Layer Shell 面板的常规 GTK 窗口；
   - 入口：托盘右键菜单项、主界面右上角齿轮图标；
   - 职责：管理主题切换（浅色/深色/跟随系统）、自定义呼出快捷键、自启动开关、白名单等。
2. **增强搜索副界面 (Enhanced Search View)**：
   - 依附于主界面，通过快捷键在“三栏工作台视图”与“全盘增强搜索视图”之间平滑切换；
   - 专注于全盘文件与深层系统内容的极速检索（后续阶段重点设计与落地）。

---

## 5. 三核心功能区模型与双向拖拽架构

### 5.1 启动台 (Launcher) 交互与模型
- **定位**：高频应用程序、脚本、常用文件与网址的极速快捷启动器。
- **数据结构**：
  ```rust
  pub struct LauncherItem {
      pub id: i64,
      pub title: String,
      pub target: String,      // 应用程序 Desktop ID 或文件绝对路径
      pub icon_name: String,   // XDG 图标名称或自定义图标路径
      pub order_index: i32,    // 排序索引
  }
  ```
- **核心交互**：
  - **左键单击**：立即启动目标应用程序或文件，**同时自动隐藏全屏面板**；
  - **右键菜单**：提供【启动】、【打开所在目录】（DBus FileManager1）、【移除该启动项】；
  - **拖动排序**：长按卡片即可在网格内拖动调整排序位置，位置变更自动持久化；
  - **外部拖入添加**：外部文件管理器拖动文件/应用图标 -> 呼出快捷键 -> 悬停在启动台区域松手 -> 启动台自动解析 `.desktop` 或文件元数据生成新卡片并落库。

### 5.2 中转站 (Stage) 核心模型与双向拖拽
- **定位**：临时文件暂存流动货架，坚持纯引用与零物理文件篡改原则。
- **数据结构**：
  ```rust
  pub struct StageItem {
      pub id: i64,                    // 内部唯一自增 ID
      pub kind: ItemKind,             // File, Image, Text
      pub uri: String,                // 规范化 file:// 或 content URI
      pub path: PathBuf,              // 本地物理路径引用
      pub display_name: String,       // 文件名或文本摘要
      pub size_bytes: u64,            // 文件大小
      pub mime_type: String,          // MIME 类型
      pub mtime_sec: u64,             // 文件最近修改时间
      pub thumbnail: Option<PathBuf>, // 异步落盘的 160px 缩略图路径
      pub pinned: bool,               // 是否锁定置顶
      pub order_index: i32,           // 拖拽排序位置
      pub created_at_ms: i64,         // 录入时间戳
  }
  ```
- **核心交互**：
  - **外部拖入 (Drag-In)**：外部拖拽目标不松手 -> 物理快捷键呼出工作台 -> 在中转站松手 -> 解析 `text/uri-list` 秒级落入中转站生成卡片；
  - **中转拖出 (Drag-Out)**：中转站拖动卡片 -> 快捷键关闭面板（通过透明代理锚点保持 Surface 存活）-> 在外部目标（文件夹、桌面、IDE）松手完成投递；
  - **左键单击**：直接将该文件路径/内容**直接粘贴到原焦点处**（自动交还原焦点 + `/dev/uinput` 硬件注入粘贴）；
  - **右键菜单**：【打开】、【粘贴】、【复制到剪贴板】、【打开所在目录】、【删除】；
  - **拖动排序**：支持卡片在货架内自由拖拽重新排位。

### 5.3 剪贴板 (Clipboard) 历史与直接消费模型
- **定位**：无感记录系统历史剪贴板，支持富文本、图片与文件列表的一键回溯消费。
- **数据结构**：
  ```rust
  pub struct ClipboardItem {
      pub id: i64,
      pub kind: ClipboardKind,        // Text, Image, FileList
      pub content: String,            // 文本内容或图片 SHA-256 寻址路径
      pub preview: String,            // UI 渲染摘要
      pub created_at_ms: i64,
  }
  ```
- **核心交互**：
  - **左键单击**：将选中内容写入系统剪贴板，立即隐藏工作台，将焦点交还至呼出前原活动窗口，通过 `/dev/uinput` 自动触发 `Ctrl+V`，实现**一键直接粘贴**；
  - **右键菜单**：【复制到剪贴板】、【删除条目】、【清空未加锁历史】。

### 5.4 虚拟滚动与视口回收机制 (Virtual Scrolling & Viewport Recycling)
- **挑战**：启动台、中转站特别是剪贴板历史，条目数极易由几十条暴增至上千条。如果按传统方式为每条数据创建 GTK Widget，会导致常驻内存膨胀（>200MB）、创建耗时长、滚动掉帧；
- **原生虚拟化方案**：
  - 界面统一采用 GTK 4 原生虚拟化列表控件：`GtkGridView`（启动台、中转站）与 `GtkListView`（剪贴板）；
  - 配合 `GListStore` 与 `GtkSignalListItemFactory`：
    - **Bind / Unbind 机制**：仅为屏幕视口当前可见的 $M$ 个卡片实例化底层 Widget（通常仅 10~25 个）；
    - 随着用户滚动，滚出视口的 Widget 被放入回收池，重新 Bind 给新进入视口的数据项；
  - 无论底层数据集有 100 条还是 10,000 条，内存中存在的 Widget 数量始终恒定，渲染开销为 $O(1)$，常驻内存牢牢锁死在 **< 40MB**。

---

## 6. 持久化与数据安全规范

- **根目录定位**：完全遵循 XDG 规范：
  - 数据文件：`$XDG_DATA_HOME/vok/vok.db`（默认 `~/.local/share/vok/vok.db`）
  - 缩略图缓存：`$XDG_CACHE_HOME/vok/thumbs/`（默认 `~/.cache/vok/thumbs/`）
  - 配置文件：`$XDG_CONFIG_HOME/vok/config.toml`（默认 `~/.config/vok/config.toml`）
- **SQLite 事务安全**：
  - 强制配置 `PRAGMA journal_mode = WAL;` 与 `PRAGMA synchronous = NORMAL;`；
  - 写入操作全部包含在显式 `BEGIN IMMEDIATE ... COMMIT` 事务中；
  - 异常崩溃只可能回滚当前未提交的一批拖入条目，绝不会破坏已有历史数据。
- **有界容量控制**：
  - 中转站最大默认容纳 **200** 个条目；
  - 缩略图缓存目录严格设定容量门槛（上限 50MB），采用 LRU 策略定期回收。

---

## 7. 极端海量条目下的性能与资源有界性保障规范

针对启动台、中转站、历史剪贴板以及未来全局增强搜索可能累积的海量条目，系统在架构上确立以下绝对保障措施：

1. **零全量 Widget 实例化 (Zero Full-Instantiation)**：
   - 视图层彻底禁用 `GtkBox` / `GtkFlowBox` 批量 append 子控件模式；
   - 必须通过 `GtkListView` / `GtkGridView` 原生视口回收工厂，保证内存中活跃 Widget 数量与总数据量完全解耦；
2. **SQLite 分页投影与懒加载 (Paged Projections & Lazy Loading)**：
   - 数据读取只按需加载前台当前视口所需字段，避免将大文本或图片 Blob 一次性加载到内存；
   - 大图与大体积剪贴板采用内容寻址散列存储在磁盘外置目录，内存中仅保留轻量级索引与元数据；
3. **事件驱动与零轮询 (Zero Polling)**：
   - 物理键态监听通过 `epoll` / 阻塞读取 `/dev/input/event*`；
   - 剪贴板监听采用系统剪贴板所有者变更通知；
   - 严禁任何形式的后台 `sleep` 轮询检查；
4. **并发隔离与单写者保证 (Serialized Storage Worker)**：
   - 所有的持久化（启动项修改、中转站存入/排序、剪贴板归档、配置保存）统一通过单写者 Storage Worker 串行执行，杜绝 UI 线程 I/O 阻塞或 SQLite 锁竞争。

---

## 8. 对 Workbench-APP 的定位与技术隔离准则

1. **成熟的交互与功能范本**：
   - Workbench-APP 沉淀的交互流（三栏横向布局、长短按呼出、外部拖拽中呼出入站、引用型货架、一键直接粘贴）在产品体验上已完全成熟并得到充分验证，本项目在**功能定义和交互流程上全面继承并对其优化**。
2. **失败的技术架构反面教材**：
   - Workbench-APP 采用的 Electron/Tauri/WebView2 多进程架构被事实证明存在不可逆的缺陷：多进程内存膨胀（>500MB）、IPC 序列化延迟导致卡顿、全屏隐藏显隐期间的 WebView 撕裂，以及为了掩盖异步竞态而充斥的大量 `sleep` 补丁代码。
3. **技术隔离铁律**：
   - 本项目在代码层面**绝对禁止直接参考或移植 Workbench-APP 的代码实现**；
   - 所有逻辑完全采用 Rust 原生单进程、GTK 4 RenderNode 渲染管线、Linux 内核 `evdev`/`uinput` 接口进行高标准实现，以纯原生质量确立极低资源与丝滑性能基线。
