# Vok (Linux)

面向 Linux 桌面环境的极速原生全屏工作台。  
*Blazing-fast native Linux fullscreen workbench for mid-drag file staging and seamless workflow.*

核心聚焦：**临时文件流转中转站 (Stage) + 外部拖拽中呼出入站 (Mid-drag Summon) + 全屏沉浸面板 (三栏 1:2:1 布局)**。

---

## 项目定位

Vok (Linux) 是专为 Linux 桌面（首发 Wayland + wlr-layer-shell，兼容 X11）打造的原生极速工作台，作为用户当前桌面之外的“第二桌面”。

在多窗口、多虚拟桌面及跨应用协同场景下，用户可随时通过全局物理热键呼出全屏面板；支持在外部任何文件管理器或应用中拖拽文件的过程中按下热键，将全屏面板作为超大中转接收区，丢入后立即隐去，彻底解决跨窗口、跨桌面拖放操作繁琐的痛点。

本项目彻底摒弃基于 Web / Electron / Tauri 的技术路线，采用纯原生 **Rust + GTK 4 + Linux 内核接口**，追求极致轻量、零白闪与工业级稳定性。

---

## 核心设计原则

1. **纯原生与极低开销**：常驻内存严格控制在 40MB 以内，无多进程渲染树与 IPC 序列化黑洞，冷呼出首帧延迟小于 16ms（1帧内）。
2. **硬件级键态感知**：基于 Linux 内核 `evdev` 直接读取硬件按键物理电平，脱离合成器事件抓取队列，确保在外部应用独占鼠标抓取拖拽时 100% 能够可靠呼出。
3. **原子全屏呈现**：Wayland 环境下通过 `wlr-layer-shell` 协议以 `OVERLAY` 层级原子呈现；X11 环境下基于 EWMH 工作区自适应贴合，杜绝黑帧、白闪与界面抽搐。
4. **引用型货架安全**：中转站仅保存文件物理路径引用与元数据快照，严禁擅自复制大文件本体、移动或修改用户原始磁盘文件。
5. **代次化状态机**：每次呼出生成唯一的单调递增 `GenerationId`。隐藏、取消、丢弃与延时回调必须严格匹配所属代次，杜绝异步脏状态污染。
6. **无盲目等待**：状态机推进由确切的 I/O 事件、内核回调或 Channel 驱动，严禁通过盲目延时（sleep）猜测合成器或窗口就绪状态。

---

## 架构分层

代码采用严格的单进程、所有权隔离三层架构：

- **vok-core (`crates/vok-core`)**：纯 Rust 状态机、领域契约、会话代次 (`GenerationId`)、热键解析模型、卡片高度整除防截断数学算法。零 UI 与零平台 FFI 依赖。
- **vok-infra (`crates/vok-infra`)**：Linux 系统基础设施适配层。封装 `evdev` 物理键鼠监听、`/dev/uinput` 虚拟击键注入、EWMH 双向前台焦点探测与返还、PolicyKit (Polkit) 原生授权、XDG 规范解析与 SQLite WAL 单写者持久化引擎。
- **vok-app (`crates/vok-app`)**：GTK 4 + Libadwaita 表现层与事件组合根。包含全屏覆盖层窗口 (`OverlayWindow`)、三栏联动视图、虚拟滚动网格 (`GtkGridView`)、独立首选项设置窗口 (`SettingsWindow`) 与 StatusNotifierItem (SNI) 系统托盘服务。

---

## 核心功能与交互范式

- **拖拽中全局呼出 (Mid-drag Summon & Drop-in)**：在文件管理器等外部程序按住鼠标左键抓取文件，半空中敲击快捷键，全屏面板瞬间揭示，释放鼠标直接完成多文件秒级入库。
- **原前台焦点暂存与原子精准返还**：呼出时暂存原活动窗口 XID 并聚焦顶部搜索框；退出时（Esc、失焦、再按热键）将焦点无感归还原前台输入控件（如浏览器搜索框、终端光标处）。
- **硬件级直接粘贴 (Direct Paste)**：中转站卡片左键单击，自动将路径写入剪贴板，平滑隐藏面板并返还焦点，通过 `/dev/uinput` 注入 Ctrl+V 击键完成直接粘贴。
- **透明代理反向拖出 (Transparent Proxy Drag-Out)**：从中转站向外部编辑器或文件管理器拖出卡片时，面板自动转换为全透明存活代理，彻底规避 Wayland 下源 Surface 隐藏掐断拖拽会话的系统级缺陷。
- **0ms 跟手瞬关与输入法完全隔离**：窗口前台以 0ms 延迟直接感知物理按键释放，长按松手手抬即隐（大于 280ms），短按常驻展示；全生命周期拦截热键修饰键，彻底阻断 Fcitx5 / IBus 状态来回跳变。
- **全盲区物理点击退让**：基于内核 `evdev` 与 XInput 2 双轨监听，覆盖包括 Ubuntu Dock 空白盲区在内的全屏任意外部点击，瞬时平滑隐藏面板，鼠标晃动 100% 免疫。

---

## 当前状态与里程碑

| 阶段 | 代号 | 核心目标 | 状态 |
| :--- | :--- | :--- | :--- |
| M0 | 宿主与拖动呼出入站内核 | evdev 物理键态状态机、避让任务栏遮罩、外部拖拽中呼出入站 | 已交付并通过实机验证 |
| M1 | 三栏骨架与双货架 MVP | 1:2:1 布局、卡片防截断高度算法、启动台与中转站、GtkGridView 虚拟滚动 | 已交付并通过 6 项量化门禁 |
| M2 | 焦点精准返还与双向消费 | 失焦自动关闭、前台焦点原子返还、/dev/uinput 直接粘贴、透明代理拖出、输入法隔离 | 已交付并通过 7 项量化门禁 |
| M3 | 剪贴板历史与三区统一搜索 | 剪贴板监听去重、顶部全局搜索三区联动过滤、增强搜索架构预留 | 下一阶段目标 |
| M4 | 独立设置与生产级打磨 | 独立设置小窗口、SNI 系统托盘常驻、权限自检引导、跨桌面打包 | 规划中（设置与托盘已前置闭环） |

详细工程实施路线与验收标准见 [docs/MILESTONES.md](docs/MILESTONES.md)。  
底层核心架构设计与线程模型见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。  
技术决策记录与被否决方案论据见 [.apc/decisions.md](.apc/decisions.md)。

---

## 环境要求与依赖安装

### 系统要求

- Linux x86_64（推荐 GNOME Wayland/X11、KDE Plasma、Hyprland 或 Sway 环境）
- Rust 工具链 1.80+ (Stable)

### 系统库依赖

- **Ubuntu / Debian**：
  ```bash
  sudo apt install -y libgtk-4-dev libadwaita-1-dev libgtk4-layer-shell-dev libevdev-dev libsqlite3-dev
  ```

- **Arch Linux**：
  ```bash
  sudo pacman -S --needed gtk4 libadwaita gtk4-layer-shell libevdev sqlite
  ```

- **Fedora**：
  ```bash
  sudo dnf install -y gtk4-devel libadwaita-devel gtk4-layer-shell-devel libevdev-devel sqlite-devel
  ```

---

## 权限配置说明

为了直接读取键盘物理电平以支持外部拖拽中全局呼出，以及通过 `/dev/uinput` 模拟直接粘贴击键，当前用户需要具备相应的设备访问权限：

1. **加入 input 用户组**：
   ```bash
   sudo usermod -aG input $USER
   ```
2. **uinput 规则（可选）**：
   若需使用直接粘贴功能，请确保 `/dev/uinput` 具备读写权限，或创建 udev 规则 `/etc/udev/rules.d/99-uinput.rules`：
   ```udev
   KERNEL=="uinput", MODE="0660", GROUP="input", OPTIONS+="static_node=uinput"
   ```
3. 重启系统或重新登录会话使权限生效。应用内置了基于 PolicyKit 的图形化授权检测与热挂载自检机制。

---

## 编译与测试

### 构建与运行

```bash
# 开发模式运行
cargo run --bin vok-app

# 编译优化版本
cargo build --release --bin vok-app

# 运行 Release 生产二进制
./target/release/vok-app
```

### 运行自动化测试套件

由于 GTK 4 主循环上下文在并发单测中的独占限制，测试套件需指定单线程执行：

```bash
cargo test --workspace -- --test-threads=1
```

---

## 授权协议

本项目基于 MIT License 协议开源，完整条款参见 [LICENSE](LICENSE)。
