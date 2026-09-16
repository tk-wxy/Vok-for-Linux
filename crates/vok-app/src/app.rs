use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crossbeam_channel::Receiver;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::Application;
use tracing::{error, info, warn};
use vok_core::{SessionAction, SessionEvent, SessionStateMachine};
use vok_infra::desktop::ThumbnailWorker;
use vok_infra::input::HotkeyEvent;
use vok_infra::storage::StorageClient;
use vok_infra::window::WindowAdapter;

use crate::overlay::OverlayWindow;

struct AppState {
    overlay: Rc<OverlayWindow>,
    state_machine: Rc<RefCell<SessionStateMachine>>,
    _tray: Option<vok_infra::TrayHandle>,
    _hold_guard: gtk4::gio::ApplicationHoldGuard,
}

pub struct VokApp {
    app: Application,
}

impl VokApp {
    pub fn new(event_rx: Receiver<HotkeyEvent>) -> Self {
        let app = Application::builder()
            .application_id("com.vok.desktop")
            .build();

        let event_rx = Arc::new(event_rx);
        let state: Rc<RefCell<Option<AppState>>> = Rc::new(RefCell::new(None));

        let state_clone = state.clone();
        app.connect_activate(move |app| {
            // 若已有实例在运行，则直接切换显隐状态 (Toggle)
            if let Some(ref current) = *state_clone.borrow() {
                let overlay = current.overlay.clone();
                let sm = current.state_machine.clone();
                let is_visible = overlay.is_visible();

                if is_visible {
                    info!("【全局快捷键/切换】工作台当前处于可见状态，执行隐藏退出...");
                    let gen = sm.borrow().current_generation();
                    let action = sm.borrow_mut().handle_event(SessionEvent::EscPressed(gen));
                    if matches!(action, SessionAction::Hide(_)) {
                        overlay.hide();
                        sm.borrow_mut().handle_event(SessionEvent::Unmapped(gen));
                        info!("【全局快捷键】工作台已成功隐藏。");
                    }
                } else {
                    info!("【全局快捷键/切换】工作台当前处于隐藏状态，执行呼出呈现...");
                    let now = std::time::Instant::now();
                    let action = sm.borrow_mut().handle_event(SessionEvent::PhysicalPress(now));
                    let _ = sm.borrow_mut().handle_event(SessionEvent::PhysicalRelease(now));
                    if let SessionAction::Reveal(gen) = action {
                        overlay.reveal(gen);
                        let commit_act = sm.borrow_mut().handle_event(SessionEvent::FrameCommitted(gen));
                        if matches!(commit_act, SessionAction::Hide(_)) {
                            overlay.hide();
                        }
                        info!("【全局快捷键】工作台已成功呈现 [{}]。", gen);
                    }
                }
                return;
            }

            info!("Vok GTK 应用激活初始化...");

            // 0. 尝试自动注册 GNOME 桌面级全局快捷键 (F12) 与静默桌面 Entry (NoDisplay)
            vok_infra::desktop::DesktopShortcut::ensure_gnome_shortcut("F12");
            vok_infra::desktop::DesktopShortcut::ensure_desktop_entry();

            // 保持 GTK 守护进程在所有窗口隐藏时不退出
            let hold_guard = app.hold();

            // 0.1 初始化持久化 SQLite WAL 存储
            let storage = match StorageClient::open_default() {
                Ok(s) => s,
                Err(e) => {
                    error!("初始化持久化存储失败: {}", e);
                    panic!("Fatal: 无法初始化 SQLite 存储: {}", e);
                }
            };

            // 0.2 启动异步缩略图后台流水线
            let thumbnail_worker = match ThumbnailWorker::start() {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    error!("启动缩略图流水线失败: {}", e);
                    panic!("Fatal: 无法启动缩略图流水线: {}", e);
                }
            };

            // 1. 获取工作区几何（避让任务栏）
            let geom = WindowAdapter::get_work_area();
            info!("初始化全屏遮罩窗口，基准尺寸: {}", geom);

            // 2. 初始化核心会话状态机
            let state_machine = Rc::new(RefCell::new(SessionStateMachine::default()));

            // 3. 构建全屏遮罩窗口
            let overlay = Rc::new(OverlayWindow::new(app, geom, storage, thumbnail_worker));

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
                            } else if let SessionAction::Hide(gen) = action {
                                info!("【热键按下】Toggle 再次触发，隐藏工作台 {}", gen);
                                overlay_for_events.hide();
                                sm.handle_event(SessionEvent::Unmapped(gen));
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

            // 8. 启动系统托盘常驻守护服务 (StatusNotifierItem)
            #[derive(Debug, Clone, Copy)]
            enum TrayCommand {
                Toggle,
                Quit,
            }

            let (tray_tx, tray_rx) = async_channel::unbounded::<TrayCommand>();
            let tx_toggle = tray_tx.clone();
            let tx_quit = tray_tx;

            let overlay_for_tray = overlay.clone();
            let sm_for_tray = state_machine.clone();
            let app_for_tray = app.clone();

            glib::spawn_future_local(async move {
                while let Ok(cmd) = tray_rx.recv().await {
                    match cmd {
                        TrayCommand::Toggle => {
                            let is_visible = overlay_for_tray.is_visible();
                            if is_visible {
                                let gen = sm_for_tray.borrow().current_generation();
                                let action = sm_for_tray.borrow_mut().handle_event(SessionEvent::EscPressed(gen));
                                if matches!(action, SessionAction::Hide(_)) {
                                    overlay_for_tray.hide();
                                    sm_for_tray.borrow_mut().handle_event(SessionEvent::Unmapped(gen));
                                    info!("【托盘操作】工作台已隐藏。");
                                }
                            } else {
                                let now = std::time::Instant::now();
                                let action = sm_for_tray.borrow_mut().handle_event(SessionEvent::PhysicalPress(now));
                                let _ = sm_for_tray.borrow_mut().handle_event(SessionEvent::PhysicalRelease(now));
                                if let SessionAction::Reveal(gen) = action {
                                    overlay_for_tray.reveal(gen);
                                    let commit_act = sm_for_tray.borrow_mut().handle_event(SessionEvent::FrameCommitted(gen));
                                    if matches!(commit_act, SessionAction::Hide(_)) {
                                        overlay_for_tray.hide();
                                    }
                                    info!("【托盘操作】工作台已呈现 [{}]。", gen);
                                }
                            }
                        }
                        TrayCommand::Quit => {
                            info!("【托盘退出】正在优雅终止 Vok 应用...");
                            app_for_tray.quit();
                        }
                    }
                }
            });

            // 8.1 注册 D-Bus 原生 Action "toggle"，支持外部 0-延时瞬时唤起
            let action_toggle = gtk4::gio::SimpleAction::new("toggle", None);
            let tx_action = tx_toggle.clone();
            action_toggle.connect_activate(move |_, _| {
                info!("【D-Bus 极速唤起】收到系统快捷键/外部 Toggle 动作");
                let _ = tx_action.send_blocking(TrayCommand::Toggle);
            });
            app.add_action(&action_toggle);

            let tray_handle = match vok_infra::VokTray::start(
                move || {
                    let _ = tx_toggle.send_blocking(TrayCommand::Toggle);
                },
                move || {
                    let _ = tx_quit.send_blocking(TrayCommand::Quit);
                },
            ) {
                Ok(h) => Some(h),
                Err(e) => {
                    warn!("启动系统托盘服务失败: {}", e);
                    None
                }
            };

            // 保存全局实例状态
            *state_clone.borrow_mut() = Some(AppState {
                overlay: overlay.clone(),
                state_machine: state_machine.clone(),
                _tray: tray_handle,
                _hold_guard: hold_guard,
            });

            // 首次启动自动呼出呈现
            let now = std::time::Instant::now();
            let initial_action = state_machine.borrow_mut().handle_event(SessionEvent::PhysicalPress(now));
            let _ = state_machine.borrow_mut().handle_event(SessionEvent::PhysicalRelease(now));
            if let SessionAction::Reveal(gen) = initial_action {
                overlay.reveal(gen);
                let _ = state_machine.borrow_mut().handle_event(SessionEvent::FrameCommitted(gen));
            }

            info!("Vok 事件路由与 GTK 主循环就绪。");
        });

        Self { app }
    }

    pub fn run(&self) -> glib::ExitCode {
        self.app.run_with_args(&Vec::<String>::new())
    }
}
