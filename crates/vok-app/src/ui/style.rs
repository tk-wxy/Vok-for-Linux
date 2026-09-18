use gtk4::gdk;

/// 安装全局深色毛玻璃 CSS 主题样式
pub fn apply_theme() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(
        "
        window.vok-window {
            background-color: rgba(18, 20, 28, 0.95);
        }

        /* 拖拽反向代理锚点态：保持 Surface 存活但全透明穿透 (规则 R3) */
        window.vok-window.vok-proxy-anchor {
            background-color: transparent !important;
        }

        .vok-root-container {
            background-color: rgba(18, 20, 28, 0.95);
            border: none;
            border-radius: 0px;
            padding: 16px 20px;
        }

        .vok-root-container.vok-proxy-anchor {
            background-color: transparent !important;
            border: none !important;
            box-shadow: none !important;
        }

        /* 顶部 Header */
        .vok-header {
            margin-bottom: 16px;
        }

        .vok-title {
            color: #ffffff;
            font-size: 18px;
            font-weight: 800;
            letter-spacing: 1.5px;
        }

        .vok-search-entry {
            min-width: 480px;
            background-color: rgba(255, 255, 255, 0.08);
            color: #ffffff;
            border: 1px solid rgba(255, 255, 255, 0.16);
            border-radius: 12px;
            padding: 8px 14px;
            font-size: 14px;
        }

        .vok-search-entry:focus-within {
            background-color: rgba(255, 255, 255, 0.12);
            border-color: #3b82f6;
        }

        .vok-settings-btn {
            background-color: rgba(255, 255, 255, 0.06);
            color: #e2e8f0;
            border: 1px solid rgba(255, 255, 255, 0.12);
            border-radius: 10px;
            padding: 6px 12px;
            font-size: 13px;
            transition: all 120ms ease;
        }

        .vok-settings-btn:hover {
            background-color: rgba(255, 255, 255, 0.12);
            border-color: rgba(255, 255, 255, 0.25);
            color: #ffffff;
        }

        .vok-status {
            color: #94a3b8;
            font-size: 13px;
        }

        /* 1:2:1 三栏容器 */
        .vok-three-pane {
            margin-top: 8px;
            margin-bottom: 12px;
        }

        .vok-pane {
            background-color: rgba(255, 255, 255, 0.03);
            border: 1px solid rgba(255, 255, 255, 0.08);
            border-radius: 16px;
            padding: 16px;
        }

        .vok-pane-launcher {
            /* 启动台 25% */
        }

        .vok-pane-stage {
            /* 中转站 50% (核心流动货架视觉加重) */
            background-color: rgba(255, 255, 255, 0.045);
            border-color: rgba(255, 255, 255, 0.12);
        }

        .vok-pane-stage.drag-hover {
            border-color: #3b82f6;
            background-color: rgba(59, 130, 246, 0.08);
        }

        .vok-pane-clipboard {
            /* 剪贴板 25% */
        }

        .vok-pane-header {
            margin-bottom: 12px;
        }

        .vok-pane-title {
            color: #f8fafc;
            font-size: 15px;
            font-weight: 700;
            letter-spacing: 0.5px;
        }

        .vok-pane-badge {
            color: #94a3b8;
            font-size: 12px;
            background-color: rgba(255, 255, 255, 0.07);
            border-radius: 8px;
            padding: 2px 8px;
        }

        /* 卡片骨架通用样式 */
        .vok-card-skeleton {
            background-color: rgba(255, 255, 255, 0.035);
            border: 1px solid rgba(255, 255, 255, 0.06);
            border-radius: 10px;
            padding: 8px 10px;
            transition: all 120ms ease;
        }

        .vok-card-skeleton:hover {
            background-color: rgba(255, 255, 255, 0.07);
            border-color: rgba(255, 255, 255, 0.2);
        }

        .vok-stage-drop-zone {
            border: 2px dashed rgba(255, 255, 255, 0.16);
            border-radius: 12px;
            padding: 16px;
            background-color: rgba(255, 255, 255, 0.02);
        }

        .vok-stage-drop-zone:hover {
            border-color: #3b82f6;
            background-color: rgba(59, 130, 246, 0.04);
        }

        /* 启动台卡片与网格 */
        .vok-launcher-grid {
            background-color: transparent;
        }

        .vok-launcher-grid > child {
            padding: 2px;
            margin: 0;
            min-width: 0;
        }

        .vok-launcher-card {
            background-color: rgba(255, 255, 255, 0.035);
            border: 1px solid rgba(255, 255, 255, 0.06);
            border-radius: 12px;
            padding: 6px 2px;
            min-height: 60px;
            transition: all 120ms ease;
        }

        .vok-launcher-card:hover {
            background-color: rgba(255, 255, 255, 0.09);
            border-color: rgba(255, 255, 255, 0.22);
        }

        .vok-launcher-card-icon {
            margin-top: 4px;
        }

        .vok-launcher-card-title {
            color: #e2e8f0;
            font-size: 11px;
            font-weight: 500;
        }

        /* 中转站网格与卡片 */
        .vok-stage-grid {
            background-color: transparent;
        }

        .vok-stage-grid > child {
            padding: 2px;
            margin: 0;
            min-width: 0;
        }

        .vok-stage-card {
            background-color: rgba(255, 255, 255, 0.035);
            border: 1px solid rgba(255, 255, 255, 0.06);
            border-radius: 14px;
            padding: 10px 14px;
            margin: 3px;
            transition: all 120ms ease;
        }

        .vok-stage-card:hover {
            background-color: rgba(255, 255, 255, 0.08);
            border-color: rgba(255, 255, 255, 0.2);
        }

        /* 置顶卡片：高亮左侧重点条 */
        .vok-stage-card-pinned {
            border-left: 3px solid #3b82f6;
            background-color: rgba(59, 130, 246, 0.06);
        }

        /* 失效/坏链卡片：虚线暗灰 */
        .vok-stage-card-broken {
            opacity: 0.6;
            border-style: dashed;
            border-color: rgba(239, 68, 68, 0.4);
        }

        .vok-stage-preview-box {
            min-width: 60px;
            min-height: 60px;
            background-color: rgba(0, 0, 0, 0.25);
            border-radius: 10px;
            padding: 4px;
        }

        .vok-stage-card-title {
            color: #f1f5f9;
            font-size: 13px;
            font-weight: 600;
        }

        .vok-stage-pin-badge {
            font-size: 11px;
            margin-left: 4px;
        }

        .vok-stage-card-meta {
            color: #94a3b8;
            font-size: 11px;
            font-weight: 400;
        }

        .vok-stage-card-path {
            color: #64748b;
            font-size: 10px;
        }

        /* 中转站清空按钮 */
        .vok-btn-clear {
            background-color: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 6px;
            color: #94a3b8;
            font-size: 11px;
            padding: 2px 8px;
        }

        .vok-btn-clear:hover {
            background-color: rgba(239, 68, 68, 0.15);
            border-color: rgba(239, 68, 68, 0.3);
            color: #fca5a5;
        }

        /* 弹出式上下文菜单 */
        popover contents {
            background-color: rgba(26, 30, 42, 0.96);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 12px;
            padding: 4px;
        }

        popover button {
            color: #e2e8f0;
            padding: 6px 12px;
            border-radius: 8px;
            font-size: 13px;
        }

        popover button:hover {
            background-color: rgba(59, 130, 246, 0.2);
            color: #ffffff;
        }

        .vok-footer {
            color: #64748b;
            font-size: 12px;
        }

        /* 快捷键录入组件 (HotkeyRecorder) */
        .vok-hotkey-recorder {
            margin: 4px 0;
        }

        .vok-hotkey-box {
            background-color: rgba(255, 255, 255, 0.06);
            border: 1px solid rgba(255, 255, 255, 0.16);
            border-radius: 10px;
            padding: 6px 12px;
            min-height: 38px;
            transition: all 120ms ease;
        }

        .vok-hotkey-box:hover {
            background-color: rgba(255, 255, 255, 0.1);
            border-color: rgba(255, 255, 255, 0.3);
        }

        .vok-hotkey-box.recording {
            background-color: rgba(59, 130, 246, 0.12);
            border-color: #3b82f6;
            box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.35);
        }

        .vok-key-badge {
            background-color: rgba(255, 255, 255, 0.12);
            border: 1px solid rgba(255, 255, 255, 0.22);
            border-radius: 6px;
            color: #ffffff;
            font-size: 12px;
            font-weight: 600;
            padding: 2px 8px;
        }

        .vok-key-badge.composing {
            background-color: rgba(59, 130, 246, 0.25);
            border-color: #3b82f6;
        }

        .vok-key-plus {
            color: #94a3b8;
            font-size: 12px;
            font-weight: 700;
            margin: 0 2px;
        }

        .vok-hotkey-hint {
            color: #94a3b8;
            font-size: 12px;
        }

        .vok-btn-reset {
            background-color: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.12);
            border-radius: 8px;
            color: #cbd5e1;
            font-size: 12px;
            padding: 6px 12px;
        }

        .vok-btn-reset:hover {
            background-color: rgba(255, 255, 255, 0.12);
            border-color: rgba(255, 255, 255, 0.25);
            color: #ffffff;
        }

        /* 设置面板浮层 (SettingsPopover) */
        .vok-settings-popover contents {
            background-color: rgba(22, 26, 36, 0.98);
            border: 1px solid rgba(255, 255, 255, 0.18);
            border-radius: 16px;
            box-shadow: 0 20px 40px rgba(0, 0, 0, 0.5);
            min-width: 380px;
        }

        .vok-settings-content {
            padding: 18px;
        }

        .vok-settings-title {
            color: #f8fafc;
            font-size: 15px;
            font-weight: 700;
            letter-spacing: 0.5px;
        }

        .vok-settings-subtitle {
            color: #94a3b8;
            font-size: 11px;
        }

        .vok-settings-section {
            background-color: rgba(255, 255, 255, 0.03);
            border: 1px solid rgba(255, 255, 255, 0.08);
            border-radius: 12px;
            padding: 12px;
        }

        .vok-settings-section-title {
            color: #e2e8f0;
            font-size: 13px;
            font-weight: 600;
        }

        .vok-settings-desc {
            color: #64748b;
            font-size: 11px;
            margin-bottom: 4px;
        }

        .vok-settings-status-box {
            background-color: rgba(0, 0, 0, 0.25);
            border: 1px solid rgba(255, 255, 255, 0.06);
            border-radius: 10px;
            padding: 10px 12px;
        }

        .vok-settings-status-ok {
            color: #10b981;
            font-size: 11px;
            font-weight: 500;
        }

        .vok-settings-conflict-hint {
            color: #38bdf8;
            font-size: 11px;
            font-weight: 500;
            margin-top: 4px;
        }

        .vok-settings-preset-label {
            color: #94a3b8;
            font-size: 11px;
            font-weight: 500;
        }

        .vok-btn-preset {
            background-color: rgba(255, 255, 255, 0.06);
            border: 1px solid rgba(255, 255, 255, 0.12);
            border-radius: 6px;
            color: #cbd5e1;
            font-size: 11px;
            font-weight: 500;
            padding: 3px 8px;
            transition: all 120ms ease;
        }

        .vok-btn-preset:hover {
            background-color: rgba(59, 130, 246, 0.2);
            border-color: #3b82f6;
            color: #ffffff;
        }

        .vok-settings-version {
            color: #475569;
            font-size: 10px;
        }
        ",
    );

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
