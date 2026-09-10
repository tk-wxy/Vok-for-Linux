# 项目清单 (Manifest)

> 已于 2026-09-10 确立，作为 Linux 原生 Vok 项目的高可信架构锚点。目标技术架构详见 `docs/ARCHITECTURE.md`，核心决策依据详见 `.apc/decisions.md`。

## 1. 使命与定位

- **名称**：Vok (Linux)
- **定位**：面向 Linux 桌面环境的极速全屏工作台，以**“临时中转站 (Stage) + 外部拖拽中呼出拖入 (Mid-drag Summon) + 全屏沉浸式面板”**为核心，提供用完即走、不打断底层工作流的第二桌面。
- **目标**：
  - 继承 `Workbench-APP` 与 Windows 版 `Vok` 验证成熟的产品形态与交互交互范式（长短按呼出、全屏盲投中转、文件引用货架、极速检索），**彻底摒弃 Web/Tauri 技术栈**，采用真正的 Linux 原生高性能技术路线；
  - 攻克 Linux (X11 & Wayland) 环境下“拖拽文件过程中全局拦截热键并安全呼出全屏面板接受拖放”的核心工程死角；
  - 实现单进程、极低资源占用（常驻内存 < 40MB，冷呼出响应 < 16ms / 1帧内），无黑帧、无闪烁、无多余后台子进程；
  - 坚持“引用型货架”原则，严禁擅自移动、修改或删除用户原文件；
  - 保证系统在长时间常驻、多次显隐、大批量文件拖放、多显示器不同缩放比下的状态确定性与数据安全。
- **非目标**：
  - 不使用 Electron / Tauri / WebView 作为核心 UI 运行时，拒绝多进程渲染与 IPC 内存黑洞；
  - 不以粗暴的 `sleep` 轮询猜测合成器与输入边沿，所有转换必须由代次因果事件（Generation）驱动；
  - 早期不构建庞杂的多进程 Broker，先保持严格线程资源所有权的单进程模型；
  - 不伪造完成度：未通过真实物理按键与真实文件管理器拖拽验证的切片不宣称完成。

---

## 2. 锁定技术栈

- **平台与架构**：Linux x86_64（首发优先支持 Wayland + `wlr-layer-shell`，兼容 X11）；
- **开发语言**：Rust (Edition 2021 / 最新 Stable)；
- **GUI 壳层**：GTK 4 + `gtk4-layer-shell`，采用 GPU 加速的 RenderNode 树与 CSS 语义主题；
- **分层设计**：
  - `vok-core`：纯 Rust 状态机、领域契约、会话代次 (`GenerationId`)，无任何 UI 与平台 FFI 依赖；
  - `vok-infra`：封装 Linux 系统基础设施（`evdev` 输入监听、`uinput` 虚拟键盘、`zbus` DBus 客户端、SQLite WAL 存储）；
  - `vok-app`：GTK4 组合根、UI 表现层与事件路由；
- **输入与物理键态**：Linux 内核 `evdev`（`/dev/input/event*`）独立高优先级监听线程，直接读取硬件物理电平，脱离 Compositor 事件捕获限制；
- **模拟输入**：Linux 内核 `/dev/uinput` 虚拟设备注入（用于直接粘贴 Ctrl+V），不依赖不受信任的合成器协议；
- **持久化**：`rusqlite`（WAL 模式、单写者、显示事务）+ `$XDG_DATA_HOME/vok` 外置资产目录；
- **系统集成**：XDG Desktop Entry、XDG Base Directory、XDG Icon Theme、`org.freedesktop.FileManager1`。

---

## 3. 架构不变量 (Invariants)

变更以下不变量必须在 `.apc/decisions.md` 中记录根因并经架构复核：

1. **零 Web 运行时**：核心交互与界面完全原生渲染，杜绝任何形式的内嵌浏览器进程树；
2. **全屏面板原子呈现**：浮层在 Wayland 下作为 `wlr-layer-shell` 的 OVERLAY 层级呈现，必须保证原子提交（Atomic Commit），严禁中间态白闪或黑帧；
3. **严格线程与所有权隔离**：
   - GTK Main Loop 拥有 UI、Window 和 Widget；
   - `evdev` 线程专职读取物理键电平并发布不可变的代次调用事件；
   - 存储 Worker 独占 SQLite 写事务与文件内容寻址哈希计算；
   - 禁止跨线程直接操作 UI 对象或在后台线程中执行无界阻塞。
4. **会话代次化 (Generation-Scoped Lifecycle)**：
   每次呼出生成唯一的 `GenerationId`。隐藏、取消、丢弃拖放与超时必须严格匹配所属代次，旧代副作用绝不允许污染新代会话；
5. **引用安全**：中转站（Stage）条目只保存文件在物理磁盘上的路径与元数据引用，不得在用户未授权时复制大文件本体或移动源文件；
6. **无盲目 Sleep**：状态机推进由确切的 I/O、协议回调或 Channel 信号驱动，严禁通过 `std::thread::sleep` 猜测系统前台或窗口呈现就绪。

---

## 4. 高风险区域 (High Risk Zones)

- **外部文件拖拽中呼出 (Mid-drag Summon)**：外部程序抓取鼠标时，全屏面板的 Surface 创建与 Enter/Motion/Drop 协议握手时序；
- **拖出窗口注销冲突 (Drag-out Unmap Bug)**：向外部应用拖出中转项时，Wayland 合成器在源 Surface 隐藏时可能强行销毁拖拽会话的兼容性防护；
- **权限与用户组边界**：`/dev/input` 与 `/dev/uinput` 的用户组权限（需确保当前用户处于 `input` 组，或提供友好的安装权限诊断引导）。
