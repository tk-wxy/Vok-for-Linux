# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: 里程碑 M1（三栏布局、启动台与中转站货架 MVP）全部 7 个纵向切片已 100% 落地并通过全部 6 项量化门禁验收，M1 正式圆满交付闭环。准备进入 M2（剪贴板监听、富文本/图片/文件货架与极速直接粘贴）。
- **Latest progress**: 完成切片 1.1 至 1.7 全量研发与量化验收。涵盖 `vok-core` 纯整除防截断数学与重排算法、`vok-infra` SQLite WAL 单写者并发存储、Freedesktop 图标与 `.desktop` 解析、DBus `FileManager1` 目录定位与异步 50MB 缓存缩略图管道、`vok-app` 1:2:1 毛玻璃深色三栏骨架与动态对齐、启动台 4 列网格（激活/删除/拖入/持久化/置顶）、中转站 2 列网格（R4 绝对引用/双向拖入拖出/右键菜单/异步缩略图回填/清空未置顶）。全工作区 51 项单元与集成测试全绿，Clippy 0 警告，Release 二进制仅 8.3MB，实机运行内存 ~4.1MB（远低于 40MB 门禁限制），50 文件批量解析持久化 1.1ms。
- **Next step**: 启动 Milestone M2（剪贴板监听、富文本/图片/文件货架与极速直接粘贴）架构分析与纵向切片拆解。
- **Known issues**: 真实硬件按键监听需用户处于 `input` 组（当前已完备支持内置 Mock 仿真）；Wayland 下后续按计划接入 `gtk4-layer-shell`。

## §0A 最近会话

### Session 2026-09-16 - 里程碑 M1（三栏布局、启动台与中转站货架 MVP）全链路落地与量化门禁闭环

- 推进并完成了切片 1.5（启动台货架）、切片 1.6（文件临时中转站货架）以及切片 1.7（全链路集成与量化门禁压测）。
- 启动台实现 4 列 `GtkGridView` 视口回收、单击启动兼自动关闭、右键上下文菜单（置顶/移除）、外部拖入与默认项自填充。
- 中转站实现 2 列 `GtkGridView` 视口回收、`GtkDropTarget` 批量文件拖入单事务落库、`GtkDragSource` 反向拖出至外部应用（`FileList`）、`ThumbnailWorker` 跨线程安全主线程回填、右键菜单（置顶/打开/目录/路径/移除）与栏头“一键清理未置顶”。
- 落地 `crates/vok-app/tests/m1_gate_test.rs` 全部 6 项量化门禁并 100% 通过：
  1. Gate 1（R9 整除防截断）：7 大分辨率矩阵测试卡片高度恒在 `[56, 140]` 且精确闭合；
  2. Gate 2（入库吞吐）：50 个文件批量解析与持久化耗时 1.1ms（远超 < 1000ms 指标）；
  3. Gate 3（R4 数据安全）：密集置顶/重排/删除/清空下，原文件 100% 二进制位不变；
  4. Gate 4（R5 WAL 并发）：100 写入与 99 并发只读查询下，`PRAGMA integrity_check = ok`；
  5. Gate 5（R8 内存指标）：Linux 实际常驻内存 VmRSS 仅 4.08MB（远低于 40MB 生产红线）；
  6. Gate 6（R3/R6/R10/R11 会话生命周期）：代次递增、透明代理锚点（`KeepProxyAnchor`）与 Esc/拖出隐藏全链路因果闭环。
- 全工作区 51 项测试全绿，Clippy 0 警告，Release 二进制仅 8.3MB，实机测试完全达标。

### Session 2026-09-16 - M1 架构分析与纵向切片规划归档

- 深入分析 M1 里程碑（三栏骨架与双货架 MVP）的核心技术约束与实现难点。
- 确立 7 大纵向实施切片（切片 1.1 领域模型与整除算法 -> 切片 1.2 SQLite WAL 存储 -> 切片 1.3 桌面服务与缩略图流水线 -> 切片 1.4 三栏骨架与动态对齐 -> 切片 1.5 启动台货架 -> 切片 1.6 中转站货架 -> 切片 1.7 门禁验收）。
- 用户确认方案，正式归档 `docs/m1_planning.md`，将最旧的 2026-09-10 会话移至 `history.md`，维护 §0A 3 次上限约束。

### Session 2026-09-12 - 启动 M0 宿主内核开发与核心状态机落地

- 批准 M0 里程碑详细实施规划（`m0_planning.md`），定义 6 大纵向切片。
- 实机检测并自动安装 Rust 1.98.1 稳定版工具链。
- 初始化 Cargo Workspace 多 crate 架构：`crates/vok-core`、`crates/vok-infra`、`crates/vok-app`。
- 实现 `vok-core`：`GenerationId` 严格代次、`HotkeyMode`（250ms 长短按判定）、`SessionStateMachine`（无 sleep、原子因果推进与旧代过滤）。
- 实现 `vok-infra`：基于 Linux 内核 `evdev` 的物理电平事件监听器与 `MockInputMonitor` 仿真通道；实现 `WindowAdapter` 基于 X11 EWMH `_NET_WORKAREA` 的任务栏避让算法。
- 实现 `vok-app`：构建 GTK 4 全屏深色毛玻璃遮罩窗口、`GtkDropTarget` 外部拖拽入站中转、Esc 与空白处点击关闭，并通过 `async-channel` + `glib::spawn_future_local` 达成零 sleep 异步事件桥接。
- 编写并全绿通过 12 项单元测试，实机成功运行并准确计算当前多屏工作区避让（`3068x1936+132+64`），Release 生产二进制仅 1.5MB。

## 指引

- 标识、技术栈、结构和命令：`manifest.md`
- 范围规则与死路：`rules.md`
- 架构选择与根因：`decisions.md`
- 目标架构与实施门禁：`../docs/ARCHITECTURE.md`
- 更早会话：`history.md`
