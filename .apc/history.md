# 归档历史 (History)

> 仅用于归档超期会话（超出 `memory.md §0A` 三次上限的历史记录）与废弃决策/规则演进追溯。日常启动请勿读取本文件。

---

## 归档会话

### Session 2026-09-10 - 初始化项目规划与建立 APC 架构体系

- 用户确认废弃原基于 Tauri/Web 的 Workbench-APP 架构，启动原生 Linux 效率桌面工作台 Vok (Linux)。
- 明确核心交互为“中转站 (Stage) + 拖拽中全局呼出并丢入 (Mid-drag Summon) + 全屏沉浸面板”。
- 确立技术选型：Rust + GTK 4 + gtk4-layer-shell + Linux 内核 evdev / uinput + SQLite WAL。
- 完整搭建 APC 知识库与规则体系：`.apc/manifest.md`、`.apc/decisions.md`、`.apc/rules.md`、`.apc/workflow.md`、`.apc/memory.md`、`docs/ARCHITECTURE.md`、`docs/MILESTONES.md` 以及根目录各 Agent 适配器文件。
