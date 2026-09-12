use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crossbeam_channel::Receiver;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::Application;
use tracing::info;
use vok_core::{SessionAction, SessionEvent, SessionStateMachine};
use vok_infra::input::HotkeyEvent;
use vok_infra::window::WindowAdapter;

use crate::overlay::OverlayWindow;

pub struct VokApp {
    app: Application,
}

impl VokApp {
    pub fn new(event_rx: Receiver<HotkeyEvent>) -> Self {
        let app = Application::builder()
            .application_id("com.vok.desktop")
            .build();

        let event_rx = Arc::new(event_rx);

        app.connect_activate(move |app| {
            info!("Vok GTK 应用激活...");

            // 1. 获取工作区几何（避让任务栏）
            let geom = WindowAdapter::get_work_area();
            info!("初始化全屏遮罩窗口，避让尺寸: {}", geom);

            // 2. 初始化核心会话状态机
            let state_machine = Rc::new(RefCell::new(SessionStateMachine::default()));

            // 3. 构建全屏遮罩窗口
            let overlay = Rc::new(OverlayWindow::new(app, geom));

            // 4. 绑定窗口 Esc / 点击外部关闭行为到状态机
            {
                let sm_clone = state_machine.clone();
                let overlay_clone = overlay.clone();
                overlay.set_on_dismiss(move || {
                    let gen = sm_clone.borrow().current_generation();
                    let action = sm_clone.borrow_mut().handle_event(SessionEvent::EscPressed(gen));
                    if matches!(action, SessionAction::Hide(_)) {
                        overlay_clone.hide();
                        sm_clone.borrow_mut().handle_event(SessionEvent::Unmapped(gen));
                        info!("工作台已退出隐藏。");
                    }
                });
            }

            // 5. 绑定拖拽丢入完成行为
            {
                let sm_clone = state_machine.clone();
                overlay.set_on_drop(move |paths| {
                    let gen = sm_clone.borrow().current_generation();
                    info!("【入站中转完成】{} 成功接住 {} 个文件", gen, paths.len());
                });
            }

            // 6. 建立后台按键事件通道与 GTK Main Loop 调度桥梁
            // 遵循架构规范：后台线程阻塞监听，通过 async-channel + glib::spawn_future_local 原生无 sleep 唤醒 UI 线程
            let (async_tx, async_rx) = async_channel::unbounded::<HotkeyEvent>();
            let event_rx_clone = event_rx.clone();

            std::thread::Builder::new()
                .name("vok-event-bridge".to_string())
                .spawn(move || {
                    while let Ok(event) = event_rx_clone.recv() {
                        if async_tx.send_blocking(event).is_err() {
                            break;
                        }
                    }
                })
                .expect("启动事件桥接线程失败");

            // 7. 在 UI 线程异步监听事件流并推进状态机
            let sm_for_events = state_machine.clone();
            let overlay_for_events = overlay.clone();

            glib::spawn_future_local(async move {
                while let Ok(event) = async_rx.recv().await {
                    let mut sm = sm_for_events.borrow_mut();
                    match event.state {
                        vok_infra::input::KeyState::Pressed => {
                            let action = sm.handle_event(SessionEvent::PhysicalPress(event.timestamp));
                            if let SessionAction::Reveal(gen) = action {
                                info!("【热键按下】呈现工作台 {}", gen);
                                overlay_for_events.reveal(gen);

                                let commit_act = sm.handle_event(SessionEvent::FrameCommitted(gen));
                                if matches!(commit_act, SessionAction::Hide(_)) {
                                    overlay_for_events.hide();
                                }
                            }
                        }
                        vok_infra::input::KeyState::Released => {
                            let action = sm.handle_event(SessionEvent::PhysicalRelease(event.timestamp));
                            if let SessionAction::Hide(gen) = action {
                                info!("【热键释放】长按 Momentary 结束，隐藏工作台 {}", gen);
                                overlay_for_events.hide();
                                sm.handle_event(SessionEvent::Unmapped(gen));
                            }
                        }
                        vok_infra::input::KeyState::Repeat => {}
                    }
                }
            });

            info!("Vok 事件路由与 GTK 主循环就绪。");
        });

        Self { app }
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.run_with_args(&Vec::<String>::new())
    }
}
