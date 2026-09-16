# 归档历史 (History)

> 仅用于归档超期会话（超出 `memory.md §0A` 三次上限的历史记录）与废弃决策/规则演进追溯。日常启动请勿读取本文件。

---

## 归档会话

### Session 2026-09-10 - 初始化项目规划与建立 APC 架构体系

- 用户确认废弃原基于 Tauri/Web 的 Workbench-APP 架构，启动原生 Linux 效率桌面工作台 Vok (Linux)。
- 明确核心交互为“中转站 (Stage) + 拖拽中全局呼出并丢入 (Mid-drag Summon) + 全屏沉浸面板”。
- 确立技术选型：Rust + GTK 4 + gtk4-layer-shell + Linux 内核 evdev / uinput + SQLite WAL。
- 完整搭建 APC 知识库与规则体系：`.apc/manifest.md`、`.apc/decisions.md`、`.apc/rules.md`、`.apc/workflow.md`、`.apc/memory.md`、`docs/ARCHITECTURE.md`、`docs/MILESTONES.md` 以及根目录各 Agent 适配器文件。

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
