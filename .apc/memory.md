# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: M1 里程碑规划归档完成，7 大纵向切片与依赖拓扑确立；准备进入 Slice 1.1 开发。
- **Latest progress**: 深入完成 M1 架构分析与切片拆解；梳理不变量、布局整除数学算法、SQLite WAL 单写者 Actor、虚拟滚动视口回收与桌面服务集成方案；正式归档 `docs/m1_planning.md`。
- **Next step**: 执行 Slice 1.1：在 `vok-core` 中落地 `LauncherItem` / `StageItem` 领域模型与 `NoClippingLayoutCalculator` 卡片防截断高度动态整除算法及纯单元测试。
- **Known issues**: 真实硬件按键监听需用户处于 `input` 组（当前已完备支持内置 Mock 仿真）；Wayland 下后续需接入 `gtk4-layer-shell` 动态编译 feature。

## §0A 最近会话

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

### Session 2026-09-12 - 总体功能规划与核心交互范式深度确立

- 用户全面补充总体功能规范：
  1. 界面与焦点：全屏覆盖避让任务栏；快捷键/托盘呼出，失焦/Esc/快捷键关闭；呼出聚焦搜索框，关闭精准返还原前台输入控件；
  2. 1:2:1 横向布局：左启动台（25%）、中中转站（50%）、右剪贴板（25%），顶部常驻联动搜索框；
  3. 两个副界面：独立设置小窗口（托盘/齿轮双入口）与全局增强搜索视图（架构预留）；
  4. 启动台：一行 4 项，卡片高度整除防截断算法，左键单击启动并隐藏，右键菜单，拖动排序，外部拖入添加；
  5. 中转站：临时文件流动货架，双向拖拽，左键单击直接粘贴，右键菜单，拖动排序，高度与启动台完全对齐；
  6. 剪贴板：文本/图片/文件捕获，单击直接粘贴，复制，删除；
  7. 性能与持久化：采用视口回收虚拟滚动（`GtkGridView`/`GtkListView`），保障海量数据下内存恒定 < 40MB、CPU 低开销，SQLite 单写者串行持久化；
  8. 定位原则：Workbench-APP 作为功能与前端成熟范本，作为技术架构失败反面教材，绝不移植其代码。
- 全量更新 `.apc/manifest.md`、`.apc/decisions.md`（追加 §7~§9）、`.apc/rules.md`（追加 R8~R11）、`docs/ARCHITECTURE.md` 与 `docs/MILESTONES.md`。

## 指引

- 标识、技术栈、结构和命令：`manifest.md`
- 范围规则与死路：`rules.md`
- 架构选择与根因：`decisions.md`
- 目标架构与实施门禁：`../docs/ARCHITECTURE.md`
- 更早会话：`history.md`
