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

---

## 5. 中转站 (Stage) 核心模型与双向拖拽架构

### 5.1 数据模型（引用型货架）
```rust
pub struct StageItem {
    pub id: i64,                    // 内部唯一自增 ID
    pub kind: ItemKind,             // File, Image, Text
    pub uri: String,                // 规范化 file:// 或 content URI
    pub path: PathBuf,              // 本地物理路径
    pub display_name: String,       // 文件名或文本摘要
    pub size_bytes: u64,            // 文件大小
    pub mime_type: String,          // MIME 类型
    pub mtime_sec: u64,             // 文件最近修改时间
    pub thumbnail: Option<PathBuf>, // 异步落盘的缩略图路径
    pub pinned: bool,               // 是否锁定置顶
    pub created_at_ms: i64,         // 录入时间戳
}
```
**铁律**：Stage 永远只存储文件的引用与状态快照，不移动、不复制大文件，绝不干涉原文件生命周期。

### 5.2 拖入 (Drag-In) 管道
1. 全屏面板背景作为全局 `GtkDropTarget`；
2. 监听 MIME 类型 `text/uri-list`；
3. 外部拖入命中后，UI 线程提取 URIs 并转化为不可变 `Vec<PathBuf>`，向 Core 发起 `AddStageEntries(GenerationId, Paths)` 命令；
4. Core 校验文件有效性，生成 `StageItem` 内存对象即时更新 UI 卡片（保证 0 延迟响应）；
5. 异步投递给 Storage Worker 写入 SQLite，并向 Thumbnail Worker 提交缩略图提取任务。

### 5.3 拖出 (Drag-Out) 协议与透明代理锚点
针对 Wayland 下“源窗口 Unmap 导致拖拽中断”的系统缺陷，采用**透明代理锚点模式 (Transparent Proxy Anchor)**：
1. 用户在卡片上拖动位移超过阈值，触发 `GtkDragSource` 启动；
2. 面板**不调用 `window.hide()`**，而是通过 CSS 类将面板背景与所有卡片置为 `opacity: 0`（全透明），并将窗口物理尺寸或交互区域收缩至最小，让出鼠标拾取权；
3. 系统原生 OLE/Wayland DnD 机制接管鼠标光标与文件投递；
4. 捕获到 `drag-end`（成功消费或取消）信号后，恢复面板状态并执行最终的隐藏收尾。

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
