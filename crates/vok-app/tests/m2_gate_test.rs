use std::time::{Duration, Instant};

use vok_core::generation::GenerationId;
use vok_core::hotkey::HotkeyMode;
use vok_core::session::{SessionAction, SessionEvent, SessionState, SessionStateMachine};
use vok_infra::window::WindowAdapter;

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
