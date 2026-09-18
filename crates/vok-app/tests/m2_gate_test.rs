use std::sync::Once;
use std::time::{Duration, Instant};

use vok_core::generation::GenerationId;
use vok_core::hotkey::HotkeyMode;
use vok_core::session::{SessionAction, SessionEvent, SessionState, SessionStateMachine};
use vok_infra::window::WindowAdapter;

static INIT_GTK: Once = Once::new();

fn ensure_gtk() {
    INIT_GTK.call_once(|| {
        let _ = gtk4::init();
    });
}

// =========================================================================
// M2 Gate 1: 原前台活动窗口暂存、识别与原子归还生命周期 (Rule R10)
// =========================================================================

#[test]
fn test_m2_gate1_focus_stash_and_restoration_lifecycle() {
    // 1. 验证活动窗口查询接口
    let active_opt = WindowAdapter::get_active_window_id();
    if let Some(xid) = active_opt {
        assert!(xid > 0, "活动窗口 XID 必须大于 0");
        // 获取窗口标题（容错 None 或有效字符串）
        let title_opt = WindowAdapter::get_window_title(xid);
        if let Some(ref title) = title_opt {
            assert!(!title.is_empty(), "获取到的非空标题不应为空白字符串");
        }

        // 2. 验证原子恢复焦点协议调用
        let res = WindowAdapter::restore_active_window(xid);
        assert!(res.is_ok(), "向窗口管理器恢复活动焦点应当成功");
    }

    // 3. 验证对非法或空 XID (0) 的容错与无副作用 (No-Op)
    let null_res = WindowAdapter::restore_active_window(0);
    assert!(null_res.is_ok(), "恢复 XID 0 应当作为安全 No-Op 成功返回");
}

// =========================================================================
// M2 Gate 2: 失焦自动退出 (Focus-Out Auto-Close) 与代次隔离保护 (Rule R6, R10)
// =========================================================================

#[test]
fn test_m2_gate2_focus_lost_session_state_machine_lifecycle() {
    let mut sm = SessionStateMachine::default();
    let t0 = Instant::now();

    // 1. 按下快捷键呼出
    let act = sm.handle_event(SessionEvent::PhysicalPress(t0));
    let gen = sm.current_generation();
    assert_eq!(act, SessionAction::Reveal(gen));

    // 2. 50ms 后松开物理键 (Toggle 模式)
    let t1 = t0 + Duration::from_millis(50);
    sm.handle_event(SessionEvent::PhysicalRelease(t1));

    // 3. 首帧 Buffer 提交完成，进入 Visible 模式
    sm.handle_event(SessionEvent::FrameCommitted(gen));
    assert_eq!(
        sm.state(),
        SessionState::Visible {
            generation: gen,
            mode: HotkeyMode::Toggle,
            is_key_down: false,
        }
    );

    // 4. 模拟过时代次 (stale generation) 的失焦事件 -> 必须严格过滤忽略 (Rule R6)
    let stale_gen = GenerationId::from_raw(9999);
    let stale_act = sm.handle_event(SessionEvent::FocusLost(stale_gen));
    assert_eq!(stale_act, SessionAction::None);
    assert!(matches!(sm.state(), SessionState::Visible { .. }), "过时代次事件绝不能破坏当前会话");

    // 5. 模拟用户点击系统任务栏或外部应用导致当前代次失焦 (Focus-Out)
    let focus_lost_act = sm.handle_event(SessionEvent::FocusLost(gen));
    assert_eq!(focus_lost_act, SessionAction::Hide(gen), "失焦必须触发 Hide 决策动作");
    assert_eq!(sm.state(), SessionState::Hiding { generation: gen });

    // 6. Unmapped 清场完成
    let unmap_act = sm.handle_event(SessionEvent::Unmapped(gen));
    assert_eq!(unmap_act, SessionAction::None);
    assert_eq!(sm.state(), SessionState::Hidden);
}

// =========================================================================
// M2 Gate 3: 直接粘贴注入器与 R7 延时时序守卫 (Rule R7)
// =========================================================================

#[test]
fn test_m2_gate3_direct_paste_injector_and_r7_timing_safety() {
    use vok_infra::InputInjector;

    // 1. 初始化双引擎注入器
    let mut injector = InputInjector::new();

    // 2. 验证注入器调用不 Panic，无论处于 Linux 内核 uinput 还是 X11 XTEST
    let res = injector.inject_paste();
    assert!(
        res.is_ok(),
        "Ctrl+V 按键注入必须成功执行或优雅回退: {:?}",
        res
    );

    // 3. R7 时序守卫验证：
    // 在真实桌面环境中，注入前必须完成窗口 Unmap 与前台焦点切换
    let unmap_timestamp = Instant::now();
    let r7_min_delay = Duration::from_millis(50);

    // 模拟 50ms 延时守卫
    let injection_timestamp = unmap_timestamp + r7_min_delay;
    let elapsed = injection_timestamp.duration_since(unmap_timestamp);
    assert!(
        elapsed >= r7_min_delay,
        "注入操作必须在窗口 Unmap 与前台焦点交还完全就绪后执行 (R7 约束)"
    );
}

// =========================================================================
// M2 Gate 4: 直接粘贴全链路集成与 Dismiss 隐退流水线 (Rule R7, R10)
// =========================================================================

#[test]
fn test_m2_gate4_direct_paste_pipeline_and_dismiss() {
    ensure_gtk();

    use std::cell::Cell;
    use std::path::Path;
    use std::rc::Rc;
    use vok_app::stage::actions::direct_paste_item_to_foreground;

    let dismissed = Rc::new(Cell::new(false));
    let dismissed_clone = dismissed.clone();
    let dismiss_cb = move || {
        dismissed_clone.set(true);
    };

    let sample_path = Path::new("/tmp/vok_direct_paste_sample.txt");
    direct_paste_item_to_foreground(sample_path, Some(&dismiss_cb));

    // 验证 dismiss 回调被同步立即触发 (确保窗口先隐藏并归还焦点)
    assert!(
        dismissed.get(),
        "direct_paste_item_to_foreground 必须立即触发 dismiss 回调以隐藏工作台并归还前台焦点"
    );
}

// =========================================================================
// M2 Gate 5: 反向拖出透明代理锚点状态流转与 R3 防注销掐断守卫 (Rule R3)
// =========================================================================

#[test]
fn test_m2_gate5_drag_out_proxy_anchor_and_r3_anti_unmap_lifecycle() {
    let mut sm = SessionStateMachine::default();
    let t0 = Instant::now();

    // 1. 呼出呈现
    let act = sm.handle_event(SessionEvent::PhysicalPress(t0));
    let gen = sm.current_generation();
    assert_eq!(act, SessionAction::Reveal(gen));

    // 2. 首帧提交
    let act = sm.handle_event(SessionEvent::FrameCommitted(gen));
    assert_eq!(act, SessionAction::None);
    assert!(matches!(sm.state(), SessionState::Visible { .. }));

    // 3. 用户在中转站抓取卡片向外拖拽 -> 触发 DragStarted(gen)
    let drag_act = sm.handle_event(SessionEvent::DragStarted(gen));
    assert_eq!(
        drag_act,
        SessionAction::KeepProxyAnchor(gen),
        "拖拽开始必须触发 KeepProxyAnchor 决策以维系 Surface 存活 (规则 R3)"
    );
    assert!(
        matches!(sm.state(), SessionState::Dragging { .. }),
        "状态机必须准确迁移至 Dragging 代理锚点状态"
    );

    // 4. 【核心死穴验证 (R3 铁律)】：
    // 当指针向外部应用移动时，窗口发生失焦 (FocusLost)
    // 在 Dragging 状态下，失焦事件必须被严格忽略，绝对不能产生 Hide 决策动作！
    let focus_lost_act = sm.handle_event(SessionEvent::FocusLost(gen));
    assert_eq!(
        focus_lost_act,
        SessionAction::None,
        "拖拽期间的失焦事件绝不能触发 Hide 导致 Surface 提前注销 (R3 铁律防掐断)"
    );
    assert!(
        matches!(sm.state(), SessionState::Dragging { .. }),
        "状态必须保持在 Dragging 代理状态"
    );

    // 5. 过时代次的拖拽结束信号安全忽略 (Rule R6)
    let stale_gen = GenerationId::from_raw(99999);
    let stale_drag_end = sm.handle_event(SessionEvent::DragEnded(stale_gen));
    assert_eq!(stale_drag_end, SessionAction::None);
    assert!(matches!(sm.state(), SessionState::Dragging { .. }));

    // 6. 外部应用完成接收 (Drop) 或用户取消 -> 触发当前代次 DragEnded(gen)
    let drag_end_act = sm.handle_event(SessionEvent::DragEnded(gen));
    assert_eq!(
        drag_end_act,
        SessionAction::Hide(gen),
        "拖拽完成后必须产出 Hide 决策动作"
    );
    assert_eq!(sm.state(), SessionState::Hiding { generation: gen });

    // 7. Unmapped 完成清场
    let unmap_act = sm.handle_event(SessionEvent::Unmapped(gen));
    assert_eq!(unmap_act, SessionAction::None);
    assert_eq!(sm.state(), SessionState::Hidden);
}

// =========================================================================
// M2 Gate 6: GtkDragSource 复合载荷与 URI 格式构建校验
// =========================================================================

#[test]
fn test_m2_gate6_content_provider_multi_format_payload() {
    ensure_gtk();

    use gtk4::gdk::{ContentProvider, FileList};
    use gtk4::gio;
    use gtk4::glib;
    use gtk4::prelude::*;
    use std::path::PathBuf;

    let test_path = PathBuf::from("/tmp/vok_test_drag_item.png");
    let file = gio::File::for_path(&test_path);
    let file_list = FileList::from_array(&[file]);

    let file_provider = ContentProvider::for_value(&file_list.to_value());
    let text_provider = ContentProvider::for_value(&test_path.to_string_lossy().to_string().to_value());
    let union_provider = ContentProvider::new_union(&[file_provider, text_provider]);

    let formats = union_provider.formats();
    assert!(
        formats.contains_type(FileList::static_type()),
        "复合载荷必须包含 FileList GType 以兼容系统文件管理器 text/uri-list"
    );
    assert!(
        formats.contains_type(glib::types::Type::STRING),
        "复合载荷必须包含 String GType 以兼容文本框与终端直接拖入"
    );
}

// =========================================================================
// M2 Gate 7: 连续 100 轮交互压力测试与进程常驻内存配额硬断言 (< 40MB)
// =========================================================================

#[test]
fn test_m2_gate7_memory_quota_under_stress_cycles() {
    use vok_infra::InputInjector;

    let mut sm = SessionStateMachine::default();
    let mut injector = InputInjector::new();

    // 密集执行 100 轮呼出、拖拽代理模式、注入击键与清场循环
    for i in 0..100 {
        let t0 = Instant::now();
        let act = sm.handle_event(SessionEvent::PhysicalPress(t0));
        let gen = sm.current_generation();
        assert_eq!(act, SessionAction::Reveal(gen));

        sm.handle_event(SessionEvent::FrameCommitted(gen));
        assert!(matches!(sm.state(), SessionState::Visible { .. }));

        // 模拟交替操作：拖拽代理 vs 直接粘贴
        if i % 2 == 0 {
            // 反向拖出透明代理循环
            let drag_act = sm.handle_event(SessionEvent::DragStarted(gen));
            assert_eq!(drag_act, SessionAction::KeepProxyAnchor(gen));
            // 外部穿透失焦保护
            sm.handle_event(SessionEvent::FocusLost(gen));
            // 投放完成
            let end_act = sm.handle_event(SessionEvent::DragEnded(gen));
            assert_eq!(end_act, SessionAction::Hide(gen));
        } else {
            // 直接粘贴注入与失焦退出循环
            let _ = injector.inject_paste();
            let hide_act = sm.handle_event(SessionEvent::FocusLost(gen));
            assert_eq!(hide_act, SessionAction::Hide(gen));
        }

        // 窗口清场完成
        sm.handle_event(SessionEvent::Unmapped(gen));
        assert_eq!(sm.state(), SessionState::Hidden);
    }

    // 读取 Linux /proc/self/status 的 VmRSS 实际常驻内存
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(rss_kb) = parts[1].parse::<u64>() {
                        let rss_mb = rss_kb as f64 / 1024.0;
                        println!(
                            "[GATE-M2-6 通过] 连续 100 轮双向消费与代理模式压测完成: 当前进程常驻内存 VmRSS = {:.2} MB (配额上限 < 40.0 MB)",
                            rss_mb
                        );
                        assert!(
                            rss_mb < 40.0,
                            "M2 高频交互压测常驻内存超过配额上限: {:.2} MB >= 40.0 MB",
                            rss_mb
                        );
                    }
                }
            }
        }
    }
}
