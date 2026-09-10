# 当前状态 (Memory)

> 低可信：将这里作为线索，依赖前须用源码或执行结果核验。

规则：
- `§0` 始终且只能包含下面四条；更新时整体覆写。
- 不要在 `§0` 中记录会话流水。
- `§0A` 最多保留三次会话；将最旧的完整条目移至 `history.md`。
- 项目标识归 `manifest.md`，范围约束归 `rules.md`，根因归 `decisions.md`。

## §0 快照

- **Status**: M0 规划与 APC 提示词工程框架建立完成；准备启动 M0 宿主内核原型构建。
- **Latest progress**: 确立 Rust + GTK 4 + gtk4-layer-shell + evdev 原生技术栈；建立 .apc/ 架构决策、规范与多 Agent 适配器体系；制定 M0~M4 里程碑。
- **Next step**: 搭建 Cargo 工作区（vok-core / vok-infra / vok-app），实现基于 evdev 的独立物理按键状态机与 GTK4 全屏浮层骨架。
- **Known issues**: 用户运行需要处于 `input` 用户组以访问 `/dev/input/event*`；首期重点解决 Wayland 下拖拽中捕获文件与呼出时序。

## §0A 最近会话

### Session 2026-09-10 - 初始化项目规划与建立 APC 架构体系

- 用户确认废弃原基于 Tauri/Web 的 Workbench-APP 架构，启动原生 Linux 效率桌面工作台 Vok (Linux)。
- 明确核心交互为“中转站 (Stage) + 拖拽中全局呼出并丢入 (Mid-drag Summon) + 全屏沉浸面板”。
- 确立技术选型：Rust + GTK 4 + gtk4-layer-shell + Linux 内核 evdev / uinput + SQLite WAL。
- 完整搭建 APC 知识库与规则体系：`.apc/manifest.md`、`.apc/decisions.md`、`.apc/rules.md`、`.apc/workflow.md`、`.apc/memory.md`、`docs/ARCHITECTURE.md`、`docs/MILESTONES.md` 以及根目录各 Agent 适配器文件。

## 指引

- 标识、技术栈、结构和命令：`manifest.md`
- 范围规则与死路：`rules.md`
- 架构选择与根因：`decisions.md`
- 目标架构与实施门禁：`../docs/ARCHITECTURE.md`
- 更早会话：`history.md`
