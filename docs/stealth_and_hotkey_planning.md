# Vok (Linux) 幽灵无感呼出、任务栏隐形与快捷键体系架构规划

> 目标：彻底抹平 Windows 版 Vok 与 Linux 版本的体验差距，消除“普通应用程序窗口”的违和感，确立“**纯净托盘常驻 + 屏幕幽灵 HUD 呼出 + 零任务栏占用 + 双轨热键与录入**”的核心交互架构。

---

## 1. 痛点诊断与设计目标

### 1.1 痛点一：快捷键是丝滑的前提（Linux 特有死角与录入诉求）
- **痛点分析**：
  - Windows 允许单进程几乎无限制注册全局键盘钩子，拖拽中依然能监听；
  - Linux 现代桌面（Wayland/XDG）出于安全沙箱机制，严格禁止常规客户端全局窃听键盘事件；
  - 在 Vok 核心交互场景——**外部拖拽中途呼出 (Mid-drag Summon)** 时，外部应用占用 Pointer/DnD 抓取，普通桌面快捷键极易被合成器吞掉；
  - 目前的快捷键是硬编码的 F12，缺乏面向用户的**可视化快捷键录入与热重载**机制。
- **设计目标**：
  - 维持双轨制保障：底层内核 `evdev`（高优先级独立线程，专攻拖拽中呼出与长短按分离）+ 桌面级 Portal/GNOME 快捷键（无权限门槛开箱即用）；
  - 设计符合桌面规范的 `HotkeyRecorder` 交互组件，支持用户直观敲击录入修饰键组合，免重启热重载。

### 1.2 痛点二：窗口呼出占用任务栏，缺乏系统托盘常驻
- **痛点分析**：
  - Windows 版 Vok 作为系统托盘应用常驻，呼出时以 ToolWindow / Overlay 形式呈现，**Windows 任务栏永远不创建新图标，不点亮运行指示，毫无侵入感**；
  - Linux 现状：当前采用常规应用顶级窗口 (`xdg-toplevel`)，窗口映射 (`present()`) 时，Ubuntu Dock（左侧任务栏）会识别为前台新应用，产生图标点亮、指示灯亮起或弹跳动效，破坏了“HUD 级第二桌面”的纯净无感体验；
  - 缺乏系统托盘（System Tray）：关闭窗口后没有直观的后台常驻锚点。
- **设计目标**：
  - **任务栏彻底隐形**：呼出时屏幕上展示三栏工作台，但左侧 Ubuntu Dock / 底部任务栏**绝无任何图标新增、绝无小白点指示、绝无动效跳动**；
  - **系统托盘常驻**：基于 Linux 事实标准 StatusNotifierItem (SNI) 协议，仅在顶部状态栏常驻一个小图标，支持左键 Toggle 呼出、右键托盘菜单与 < 5MB 内存静默守护。

---

## 2. 核心技术方案设计

```
                         ┌─────────────────────────────┐
                         │   Linux 桌面系统环境        │
                         └──────────────┬──────────────┘
                                        │
             ┌──────────────────────────┴──────────────────────────┐
             ▼                                                     ▼
┌─────────────────────────┐                             ┌─────────────────────────┐
│  快捷键体系 (Hotkey)    │                             │  幽灵窗口与托盘 (Stealth)│
└────────────┬────────────┘                             └────────────┬────────────┘
             │                                                     │
   ┌─────────┴─────────┐                                 ┌─────────┴─────────┐
   ▼                   ▼                                 ▼                   ▼
【内核级 evdev】    【桌面级 Portal/D-Bus】           【任务栏彻底隐形】   【StatusNotifierItem 托盘】
- 直接读取原始电平  - GNOME/XDG 桌面原生              - _NET_WM_STATE_     - 顶部面板常驻小图标
- 拖拽中 100% 触发  - 免 root / 免 input 组             SKIP_TASKBAR       - 左键单击 Toggle 显隐
- 长短按因果状态机  - 单实例 D-Bus Toggle             - _NET_WM_WINDOW_    - 右键菜单 (呼出/设置/退出)
                       │                                TYPE_UTILITY       - 关闭窗口转入静默后台
                       ▼                                   │
             【快捷键录入组件】                            ▼
             - GTK4 按键捕获                     【Layer Shell (Wayland)】
             - 格式化为 <Super>Space             - 挂载 OVERLAY 协议层级
             - SQLite 持久化 + 动态热更新        - 天生脱离任务栏管理
```

### 2.1 任务栏隐形 (Skip Taskbar) 机制

GNOME Shell 与 Ubuntu Dock 的窗口过滤逻辑核心是 `Shell.App` 与 `MetaWindow` 的 `skip_taskbar` 属性：
1. **X11 / XWayland 方案**：
   - 窗口创建并映射时，为 X11 窗口句柄附加：
     - `_NET_WM_STATE_SKIP_TASKBAR`（跳过任务栏）
     - `_NET_WM_STATE_SKIP_PAGER`（跳过工作区切换器）
     - `_NET_WM_WINDOW_TYPE` 设置为 `_NET_WM_WINDOW_TYPE_UTILITY` 或 `_NET_WM_WINDOW_TYPE_DOCK`；
   - 一旦满足该状态，Ubuntu Dock 会执行 `windows.filter(w => !w.skip_taskbar)`，得到的有效窗口数为 0，**彻底不在任务栏显示图标或运行指示灯**。
2. **Wayland 原生 Layer Shell 路线**：
   - 使用 `gtk4-layer-shell` 将窗口注册为 `ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY`；
   - 协议层脱离常规应用窗口树，合成器与 Dock 根本不把 Layer Surface 作为任务栏项管理。

### 2.2 系统托盘 (StatusNotifierItem) 机制

- **协议标准**：采用现代桌面通用的 D-Bus **StatusNotifierItem (SNI)** 规范（D-Bus 接口 `org.kde.StatusNotifierItem`，由 `org.kde.StatusNotifierWatcher` 托管，Ubuntu GNOME 自带 AppIndicator 扩展原生支持）；
- **行为规范**：
  - 应用启动时：创建托盘图标，默认主窗口保持 `visible = false`；
  - 托盘图标单击：触发 `Toggle` 呼出/收起；
  - 托盘右键菜单：
    - 🚀 呼出工作台 (`F12`)
    - ⚙️ 设置...
    - ℹ️ 关于 Vok
    - ❌ 退出
  - 窗口内按 Esc / 点击空白处：窗口隐藏，进程不退出，仅托盘保持静默运行。

### 2.3 快捷键体系与快捷键录入 (Hotkey Recorder) 架构

1. **底层双轨分工**：
   - **`evdev`**：负责高精度的“长按 250ms 临时呼出，松手即隐（Momentary）”以及“拖拽中呼出”；
   - **`GNOME/Portal D-Bus`**：负责常规桌面环境下无权限门槛的单键/组合键快速呼出（Toggle）。
2. **快捷键自定义录入组件 (`HotkeyRecorder`)**：
   - **数据结构**：
     ```rust
     pub struct HotkeyConfig {
         pub modifiers: u32,       // Ctrl, Alt, Shift, Super 标志位
         pub key_code: u32,        // 物理按键代码
         pub display_text: String, // 友好的 UI 展示，如 "Super + Space"
         pub xdg_shortcut: String, // XDG/GNOME 格式，如 "<Super>space"
     }
     ```
   - **录入交互状态机**：
     - `Idle`：显示当前快捷键标签，带“修改”按钮；
     - `Recording`：获得焦点，高亮边框，提示“请按下新的快捷键组合（按 Esc 取消）”；
     - `Evaluating`：监听按键按下与修饰键掩码，过滤无意义单修饰键（如仅按 Shift）；
     - `Committed`：合法组合键输入完成，写入持久化数据库，同时向系统 D-Bus 发送解绑旧键与绑定新键信号。

---

## 3. 纵向切片实施规划 (Implementation Slices)

为保证可验证性与工程可控，拆解为 3 个递进切片：

### 切片 S1：任务栏彻底隐形（X11 EWMH 属性闭环）
- **范围**：
  - 在 `vok-infra::window` 中扩展 X11 窗口属性注入方法 `set_skip_taskbar(window_id)`；
  - 在 `vok-app` 窗口构建与首次映射时调用，注入 `_NET_WM_STATE_SKIP_TASKBAR` 与 `_NET_WM_STATE_SKIP_PAGER`；
- **验收指标**：
  - 呼出工作台时，Ubuntu Dock（左侧任务栏）不显示 Vok 图标，不点亮运行小点，不闪烁；
  - 关闭工作台时，桌面任务栏保持绝对纯净。

### 切片 S2：StatusNotifierItem 系统托盘常驻与生命周期闭环
- **范围**：
  - 在 `vok-infra` 引入轻量级 StatusNotifierItem 实现（通过 `zbus` 注册 `org.kde.StatusNotifierItem` 或集成成熟纯 Rust 托盘抽象）；
  - 托盘常驻：在 Ubuntu 顶栏显示 Vok 图标；
  - 绑定交互：左键单击 Toggle 显隐主窗口，右键弹出包含【呼出】、【关于】、【退出】的上下文菜单；
  - 重构主循环退出逻辑：窗口关闭仅 `hide()`，只有点击托盘菜单【退出】才彻底终止进程。
- **验收指标**：
  - 启动后仅在顶栏托盘显示图标；
  - 点击托盘与按 F12 均能丝滑呼出/隐藏；
  - 常驻后台内存维持在 `< 5MB`。

### 切片 S3：快捷键领域模型与录入组件设计
- **范围**：
  - 在 `vok-core` 建立 `HotkeyConfig` 领域契约；
  - 在 `vok-infra` 扩展 D-Bus 快捷键动态重载方法；
  - 设计 `HotkeyRecorder` UI 组件（预置在右上角设置或独立对话框中）；
- **验收指标**：
  - 用户敲击组合键可准确识别并格式化为标准描述符；
  - 动态更换快捷键后免重启实时生效。

---

## 4. 与后续 M2 里程碑的承接关系

1. 完成 **切片 S1（任务栏隐形）** 与 **切片 S2（系统托盘常驻）** 后，Vok 在 Linux 上的“幽灵 HUD”产品质感将与 Windows 版完全一致；
2. 随后即可在干净优雅的宿主基础上，全力推进 **M2 里程碑**（原前台失焦精准返还、`/dev/uinput` 硬件直接粘贴、透明代理拖出外部）。
