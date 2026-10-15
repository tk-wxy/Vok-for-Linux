# Vok (Linux)

> **极速、原生、用完即走的 Linux 全屏桌面工作台**  
> 核心聚焦：**临时中转站 (Stage) + 拖拽中全局呼出并丢入 (Mid-drag Summon) + 全屏沉浸面板**

---

## 🎯 项目定位

Vok (Linux) 是专为 Linux 桌面（Wayland & X11）打造的原生极速工作台。它作为用户当前桌面之外的“第二桌面”，通过全局物理热键随时唤出；支持在外部任何应用拖拽文件的过程中按下热键，将全屏面板作为超大中转接收区，丢入后立即隐去，彻底解决多窗口与多桌面之间的跨屏拖放痛点。

本项目彻底废弃前作（基于 Web/Tauri）的技术路线，采用纯原生 **Rust + GTK 4 + Linux 内核接口**，追求极致轻量、零白闪与工业级稳定性。

---

## ⚡ 核心设计原则

1. **纯原生、低开销**：常驻内存严格控制在 **< 40MB**，无浏览器多进程，呼出响应 < 16ms（1帧内）；
2. **硬件级键态感知**：基于内核 `evdev` 直接读取硬件按键物理电平，脱离 Compositor 事件队列，确保在任何拖拽抓取循环中 100% 可呼出；
3. **原子全屏呈现**：在 Wayland 下通过 `wlr-layer-shell` 协议以 `OVERLAY` 层级呈现，无黑帧、无闪烁、不破坏系统工作区；
4. **引用型货架安全**：中转站仅保存文件路径引用与元数据快照，绝不擅自移动、修改或删除用户原文件；
5. **代次化状态机**：每次呼出严格绑定唯一的 `GenerationId`，杜绝时序猜测与脏状态回流。

---

## 🛠️ 技术栈

- **开发语言**：Rust (Edition 2021)
- **GUI 框架**：GTK 4 + `gtk4-layer-shell` (Wayland 原生层级支持)
- **底层输入**：Linux 内核 `evdev` (物理按键监测) + `/dev/uinput` (虚拟键盘注入)
- **本地存储**：`rusqlite` (WAL 模式强事务) + `$XDG_DATA_HOME/vok`
- **系统集成**：XDG 规范 (Desktop Entry, Icon Theme, Mime Apps) + DBus (`zbus`)

---

## 📅 当前状态与里程碑

- **已交付阶段**：
  - **M0 宿主与拖拽入站内核**：物理键态状态机、全屏遮罩、焦点暂存、外部拖拽中呼出入站（已交付并实机验证）；
  - **M1 三栏骨架与双货架**：1:2:1 布局、卡片防截断高度算法、启动台与中转站、虚拟化视口（已交付并全绿通过 6 项量化门禁）；
  - **M2 焦点精准返还与双向消费**：失焦退出、前台焦点原子返还、`/dev/uinput` 硬件直接粘贴、透明代理反向拖出、全盲区硬件点击感知、0ms 极致跟手瞬关与输入法完全隔离（已交付并全绿通过 7 项量化门禁）；
- **下一阶段目标**：**M3 — 剪贴板历史与三区统一联动搜索**；
- 详细实施路线与阶段门禁见 [docs/MILESTONES.md](docs/MILESTONES.md)；
- 系统架构与线程模型设计见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)；
- 底层技术决策与依据见 [.apc/decisions.md](.apc/decisions.md)。

---

## 📦 环境要求与依赖

* Linux x86_64（推荐运行在 Sway, Hyprland, KDE Plasma 或 GNOME Wayland 环境下）
* Rust 工具链（1.80+）
* 系统开发库依赖：
  ```bash
  # Ubuntu / Debian
  sudo apt install libgtk-4-dev libgtk4-layer-shell-dev libevdev-dev libsqlite3-dev
  
  # Arch Linux
  sudo pacman -S gtk4 gtk4-layer-shell libevdev sqlite
  
  # Fedora
  sudo dnf install gtk4-devel gtk4-layer-shell-devel libevdev-devel sqlite-devel
  ```
* 用户权限：当前运行用户需处于 `input` 用户组以访问 `/dev/input/event*`：
  ```bash
  sudo usermod -aG input $USER
  ```

---

## 📄 授权协议

本项目采用 MIT 协议开源。
