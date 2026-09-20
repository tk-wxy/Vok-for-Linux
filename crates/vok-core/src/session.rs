use std::time::{Duration, Instant};

use crate::generation::{GenerationId, GenerationIssuer};
use crate::hotkey::{DEFAULT_TAP_MAX_MS, HotkeyMode};

/// 会话状态定义
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// 完全隐藏状态，无活跃会话
    Hidden,
    /// 揭示呈现中（已发出呈现指令，等待 GTK 首帧 Buffer 提交）
    Revealing {
        generation: GenerationId,
        press_instant: Instant,
        released_early: Option<Duration>,
    },
    /// 已完全呈现并捕获焦点
    Visible {
        generation: GenerationId,
        press_instant: Instant,
        mode: HotkeyMode,
        is_key_down: bool,
    },
    /// 外部拖拽交互中（拖出时透明代理锚点保持存活）
    Dragging {
        generation: GenerationId,
        previous_mode: HotkeyMode,
    },
    /// 隐藏与清理中（正在注销或交还焦点）
    Hiding {
        generation: GenerationId,
    },
}

/// 会话输入事件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    /// 物理热键按下
    PhysicalPress(Instant),
    /// 物理热键释放
    PhysicalRelease(Instant),
    /// UI 首帧渲染提交就绪
    FrameCommitted(GenerationId),
    /// 拖出操作启动
    DragStarted(GenerationId),
    /// 拖出操作完成或被取消
    DragEnded(GenerationId),
    /// 用户按下 Esc 键
    EscPressed(GenerationId),
    /// 面板失去焦点（如点击任务栏或其他窗口）
    FocusLost(GenerationId),
    /// 窗口完全 Unmap 清场完成
    Unmapped(GenerationId),
}

/// 状态机产出的决策动作
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAction {
    /// 无需动作
    None,
    /// 指示 UI 线程揭示浮层并请求焦点
    Reveal(GenerationId),
    /// 指示 UI 线程隐藏浮层并交还焦点
    Hide(GenerationId),
    /// 指示 UI 线程保持透明代理锚点存活（针对 Wayland 拖出保护）
    KeepProxyAnchor(GenerationId),
}

/// 纯领域会话状态机
/// 
/// 严格遵循架构规范：
/// 1. 每次呼出单调递增派发 GenerationId；
/// 2. 区分长按瞬态（Momentary）与短按翻转（Toggle）；
/// 3. 无任何阻塞式 sleep；
/// 4. 自动过滤过期代次的事件与动作。
#[derive(Debug)]
pub struct SessionStateMachine {
    state: SessionState,
    issuer: GenerationIssuer,
    tap_threshold: Duration,
}

impl Default for SessionStateMachine {
    fn default() -> Self {
        Self::new(Duration::from_millis(DEFAULT_TAP_MAX_MS))
    }
}

impl SessionStateMachine {
    /// 使用指定的长短按时间阈值初始化状态机
    pub fn new(tap_threshold: Duration) -> Self {
        Self {
            state: SessionState::Hidden,
            issuer: GenerationIssuer::new(),
            tap_threshold,
        }
    }

    /// 获取当前会话状态
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// 获取当前生效的代次（若处于 Hidden 则为最新分配或初始代次）
    pub fn current_generation(&self) -> GenerationId {
        match self.state {
            SessionState::Hidden => self.issuer.current(),
            SessionState::Revealing { generation, .. }
            | SessionState::Visible { generation, .. }
            | SessionState::Dragging { generation, .. }
            | SessionState::Hiding { generation } => generation,
        }
    }

    /// 处理会话事件并返回确定性动作
    pub fn handle_event(&mut self, event: SessionEvent) -> SessionAction {
        match (self.state, event) {
            // 1. 隐藏状态下按下按键 -> 触发全新会话揭示
            (SessionState::Hidden, SessionEvent::PhysicalPress(instant)) => {
                let gen = self.issuer.next();
                self.state = SessionState::Revealing {
                    generation: gen,
                    press_instant: instant,
                    released_early: None,
                };
                SessionAction::Reveal(gen)
            }

            // 2. 揭示期间提前松手（短按或快速按键）
            (
                SessionState::Revealing {
                    generation,
                    press_instant,
                    released_early: _,
                },
                SessionEvent::PhysicalRelease(release_instant),
            ) => {
                let duration = release_instant.saturating_duration_since(press_instant);
                self.state = SessionState::Revealing {
                    generation,
                    press_instant,
                    released_early: Some(duration),
                };
                SessionAction::None
            }

            // 3. 揭示期间首帧渲染提交就绪
            (
                SessionState::Revealing {
                    generation,
                    press_instant,
                    released_early,
                },
                SessionEvent::FrameCommitted(gen),
            ) => {
                if gen != generation {
                    // 忽略过时甚至越代的帧提交
                    return SessionAction::None;
                }

                if let Some(duration) = released_early {
                    let mode = HotkeyMode::from_duration(duration, self.tap_threshold);
                    if mode == HotkeyMode::Momentary {
                        // 如果长按在首帧到达前已经松手，立即退回隐藏
                        self.state = SessionState::Hiding { generation };
                        SessionAction::Hide(generation)
                    } else {
                        // 短按松手后，进入 Visible 模式常驻展示
                        self.state = SessionState::Visible {
                            generation,
                            press_instant,
                            mode: HotkeyMode::Toggle,
                            is_key_down: false,
                        };
                        SessionAction::None
                    }
                } else {
                    // 核心修复：首帧渲染提交到达时（通常只需 ~4.7ms），人手必然仍处于按下状态。
                    // 严禁在此处武断定性为 Momentary！保持 is_key_down = true 并保留 press_instant，
                    // 待物理松开按键 (PhysicalRelease) 时再根据按住总时长进行权威判决。
                    self.state = SessionState::Visible {
                        generation,
                        press_instant,
                        mode: HotkeyMode::Toggle,
                        is_key_down: true,
                    };
                    SessionAction::None
                }
            }

            // 4. 显示中松手：权威决断长按 (Momentary) 与短按 (Toggle)
            (
                SessionState::Visible {
                    generation,
                    press_instant,
                    is_key_down: true,
                    ..
                },
                SessionEvent::PhysicalRelease(release_instant),
            ) => {
                let duration = release_instant.saturating_duration_since(press_instant);
                let mode = HotkeyMode::from_duration(duration, self.tap_threshold);

                if mode == HotkeyMode::Momentary {
                    // 超过长按阈值（例如 > 280ms）：长按瞬态模式，松手即隐藏！
                    self.state = SessionState::Hiding { generation };
                    SessionAction::Hide(generation)
                } else {
                    // 小于等于长按阈值（例如 <= 280ms）：短按翻转模式，松手保持常驻展示！
                    self.state = SessionState::Visible {
                        generation,
                        press_instant,
                        mode: HotkeyMode::Toggle,
                        is_key_down: false,
                    };
                    SessionAction::None
                }
            }

            // 5. 显示中再次按下物理键（Toggle 关闭）
            (
                SessionState::Visible {
                    generation,
                    mode: HotkeyMode::Toggle,
                    is_key_down: false,
                    ..
                },
                SessionEvent::PhysicalPress(_),
            ) => {
                self.state = SessionState::Hiding { generation };
                SessionAction::Hide(generation)
            }

            // 6. 显示中按下 Esc 键退出
            (SessionState::Visible { generation, .. }, SessionEvent::EscPressed(gen))
            | (SessionState::Revealing { generation, .. }, SessionEvent::EscPressed(gen)) => {
                if gen == generation {
                    self.state = SessionState::Hiding { generation };
                    SessionAction::Hide(generation)
                } else {
                    SessionAction::None
                }
            }

            // 7. 显示中失焦退出（点击外部或任务栏）
            (SessionState::Visible { generation, .. }, SessionEvent::FocusLost(gen))
            | (SessionState::Revealing { generation, .. }, SessionEvent::FocusLost(gen)) => {
                if gen == generation {
                    self.state = SessionState::Hiding { generation };
                    SessionAction::Hide(generation)
                } else {
                    SessionAction::None
                }
            }

            // 8. 拖拽启动（进入透明代理模式）
            (
                SessionState::Visible {
                    generation, mode, ..
                },
                SessionEvent::DragStarted(gen),
            ) => {
                if gen == generation {
                    self.state = SessionState::Dragging {
                        generation,
                        previous_mode: mode,
                    };
                    SessionAction::KeepProxyAnchor(generation)
                } else {
                    SessionAction::None
                }
            }

            // 9. 拖拽结束（无论是投递成功还是取消）
            (
                SessionState::Dragging { generation, .. },
                SessionEvent::DragEnded(gen),
            ) => {
                if gen == generation {
                    self.state = SessionState::Hiding { generation };
                    SessionAction::Hide(generation)
                } else {
                    SessionAction::None
                }
            }

            // 10. 窗口 Unmap 清场完成 -> 回到 Hidden
            (SessionState::Hiding { generation }, SessionEvent::Unmapped(gen)) => {
                if gen == generation {
                    self.state = SessionState::Hidden;
                }
                SessionAction::None
            }

            // 其他未匹配或过期事件安全忽略
            _ => SessionAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_tap_toggle_lifecycle() {
        let mut sm = SessionStateMachine::default();
        assert_eq!(sm.state(), SessionState::Hidden);

        let t0 = Instant::now();
        // 1. 按下按键
        let act = sm.handle_event(SessionEvent::PhysicalPress(t0));
        let gen = sm.current_generation();
        assert_eq!(act, SessionAction::Reveal(gen));
        assert!(matches!(sm.state(), SessionState::Revealing { .. }));

        // 2. 50ms 后松开（短按）
        let t1 = t0 + Duration::from_millis(50);
        let act = sm.handle_event(SessionEvent::PhysicalRelease(t1));
        assert_eq!(act, SessionAction::None);

        // 3. 首帧 Buffer 提交到达
        let act = sm.handle_event(SessionEvent::FrameCommitted(gen));
        assert_eq!(act, SessionAction::None);
        assert_eq!(
            sm.state(),
            SessionState::Visible {
                generation: gen,
                press_instant: t0,
                mode: HotkeyMode::Toggle,
                is_key_down: false,
            }
        );

        // 4. 再次按下 -> 触发隐藏 Toggle
        let t2 = t1 + Duration::from_millis(500);
        let act = sm.handle_event(SessionEvent::PhysicalPress(t2));
        assert_eq!(act, SessionAction::Hide(gen));
        assert_eq!(sm.state(), SessionState::Hiding { generation: gen });

        // 5. Unmapped 完成
        let act = sm.handle_event(SessionEvent::Unmapped(gen));
        assert_eq!(act, SessionAction::None);
        assert_eq!(sm.state(), SessionState::Hidden);
    }

    #[test]
    fn test_long_press_momentary_lifecycle() {
        let mut sm = SessionStateMachine::default();
        let t0 = Instant::now();

        // 1. 按下按键呼出
        let act = sm.handle_event(SessionEvent::PhysicalPress(t0));
        let gen = sm.current_generation();
        assert_eq!(act, SessionAction::Reveal(gen));

        // 2. 首帧提交（此时按键仍未松开）
        let act = sm.handle_event(SessionEvent::FrameCommitted(gen));
        assert_eq!(act, SessionAction::None);
        assert_eq!(
            sm.state(),
            SessionState::Visible {
                generation: gen,
                press_instant: t0,
                mode: HotkeyMode::Toggle,
                is_key_down: true,
            }
        );

        // 3. 用户在拖拽中持续按住了 300ms，随后松开
        let t1 = t0 + Duration::from_millis(300);
        let act = sm.handle_event(SessionEvent::PhysicalRelease(t1));
        // Momentary 松手即隐藏！
        assert_eq!(act, SessionAction::Hide(gen));
        assert_eq!(sm.state(), SessionState::Hiding { generation: gen });
    }

    #[test]
    fn test_real_world_short_tap_after_frame_committed() {
        let mut sm = SessionStateMachine::default();
        let t0 = Instant::now();

        // 1. 按下按键呼出 (t=0)
        let act = sm.handle_event(SessionEvent::PhysicalPress(t0));
        let gen = sm.current_generation();
        assert_eq!(act, SessionAction::Reveal(gen));

        // 2. 首帧渲染极速提交 (t=5ms，真实场景 ~4.7ms)
        let _t_frame = t0 + Duration::from_millis(5);
        let act = sm.handle_event(SessionEvent::FrameCommitted(gen));
        assert_eq!(act, SessionAction::None);
        // 此时窗口已可见，但由于人类敲击按键尚未松开，is_key_down 为 true
        assert_eq!(
            sm.state(),
            SessionState::Visible {
                generation: gen,
                press_instant: t0,
                mode: HotkeyMode::Toggle,
                is_key_down: true,
            }
        );

        // 3. 用户正常敲击短按，在 80ms 时松开按键 (t=80ms <= 280ms)
        let t_release = t0 + Duration::from_millis(80);
        let act = sm.handle_event(SessionEvent::PhysicalRelease(t_release));
        // 关键断言：绝对不能隐藏！必须维持 Visible 并保持 Toggle 模式，且 is_key_down 变为 false
        assert_eq!(act, SessionAction::None);
        assert_eq!(
            sm.state(),
            SessionState::Visible {
                generation: gen,
                press_instant: t0,
                mode: HotkeyMode::Toggle,
                is_key_down: false,
            }
        );
    }

    #[test]
    fn test_esc_key_closes_visible_panel() {
        let mut sm = SessionStateMachine::default();
        let t0 = Instant::now();
        sm.handle_event(SessionEvent::PhysicalPress(t0));
        let gen = sm.current_generation();
        sm.handle_event(SessionEvent::FrameCommitted(gen));

        let act = sm.handle_event(SessionEvent::EscPressed(gen));
        assert_eq!(act, SessionAction::Hide(gen));
        assert_eq!(sm.state(), SessionState::Hiding { generation: gen });
    }

    #[test]
    fn test_focus_lost_closes_visible_panel() {
        let mut sm = SessionStateMachine::default();
        let t0 = Instant::now();
        sm.handle_event(SessionEvent::PhysicalPress(t0));
        let gen = sm.current_generation();
        sm.handle_event(SessionEvent::FrameCommitted(gen));

        let act = sm.handle_event(SessionEvent::FocusLost(gen));
        assert_eq!(act, SessionAction::Hide(gen));
        assert_eq!(sm.state(), SessionState::Hiding { generation: gen });
    }

    #[test]
    fn test_drag_out_proxy_anchor_and_completion() {
        let mut sm = SessionStateMachine::default();
        let t0 = Instant::now();
        sm.handle_event(SessionEvent::PhysicalPress(t0));
        let gen = sm.current_generation();
        sm.handle_event(SessionEvent::FrameCommitted(gen));

        // 开始向外部拖拽卡片
        let act = sm.handle_event(SessionEvent::DragStarted(gen));
        assert_eq!(act, SessionAction::KeepProxyAnchor(gen));
        assert!(matches!(sm.state(), SessionState::Dragging { .. }));

        // 外部应用接收释放并完成
        let act = sm.handle_event(SessionEvent::DragEnded(gen));
        assert_eq!(act, SessionAction::Hide(gen));
        assert_eq!(sm.state(), SessionState::Hiding { generation: gen });
    }

    #[test]
    fn test_stale_generation_events_are_ignored() {
        let mut sm = SessionStateMachine::default();
        let t0 = Instant::now();
        sm.handle_event(SessionEvent::PhysicalPress(t0));
        let gen1 = sm.current_generation();

        let stale_gen = GenerationId::from_raw(999);
        assert_ne!(gen1, stale_gen);

        // 过期代次的 Esc 或 FrameCommitted 绝不能影响当前会话
        let act = sm.handle_event(SessionEvent::EscPressed(stale_gen));
        assert_eq!(act, SessionAction::None);
        assert!(matches!(sm.state(), SessionState::Revealing { .. }));

        let act = sm.handle_event(SessionEvent::FrameCommitted(stale_gen));
        assert_eq!(act, SessionAction::None);
        assert!(matches!(sm.state(), SessionState::Revealing { .. }));
    }
}
