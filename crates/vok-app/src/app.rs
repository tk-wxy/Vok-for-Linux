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
use crate::ui::{SettingsWindow, ThemeManager};

struct AppState {
    _overlay: Rc<OverlayWindow>,
    _settings_window: Rc<SettingsWindow>,
    _theme_manager: ThemeManager,
    _state_machine: Rc<RefCell<SessionStateMachine>>,
    _tray: Option<vok_infra::TrayHandle>,
    _hold_guard: gtk4::gio::ApplicationHoldGuard,
}

pub struct VokApp {
    app: Application,
}

impl VokApp {
    pub fn new(
        event_rx: Receiver<HotkeyEvent>,
        evdev_monitor: Option<Arc<vok_infra::EvdevMonitor>>,
        storage: StorageClient,
    ) -> Self {
        let app = Application::builder()
            .application_id("com.vok.desktop")
            .build();

        let event_rx = Arc::new(event_rx);
        let state: Rc<RefCell<Option<AppState>>> = Rc::new(RefCell::new(None));
        let last_state_change: Rc<RefCell<Option<std::time::Instant>>> =
            Rc::new(RefCell::new(None));

        #[derive(Debug)]
        struct DbusHoldDetector {
            pending_hold_source: Option<glib::SourceId>,
            silence_source: Option<glib::SourceId>,
            reveal_instant: Option<std::time::Instant>,
            is_holding: bool,
        }

        impl DbusHoldDetector {
            fn new() -> Self {
                Self {
                    pending_hold_source: None,
                    silence_source: None,
                    reveal_instant: None,
                    is_holding: false,
                }
            }

            fn cancel_all(&mut self) {
                if let Some(s) = self.pending_hold_source.take() {
                    s.remove();
                }
                if let Some(s) = self.silence_source.take() {
                    s.remove();
                }
                self.reveal_instant = None;
                self.is_holding = false;
            }
        }

        let dbus_hold_detector: Rc<RefCell<DbusHoldDetector>> =
            Rc::new(RefCell::new(DbusHoldDetector::new()));

        #[derive(Debug, Clone, Copy)]
        enum TrayCommand {
            Toggle,
            OpenSettings,
            Quit,
        }

        let tray_tx_cell: Rc<RefCell<Option<async_channel::Sender<TrayCommand>>>> =
            Rc::new(RefCell::new(None));
        let tray_tx_for_act = tray_tx_cell.clone();

        let state_clone = state.clone();
        let evdev_clone_for_act = evdev_monitor.clone();
        let storage_clone_for_act = storage.clone();
        let detector_clone_for_act = dbus_hold_detector;

        app.connect_activate(move |app| {
            // 若已有实例在运行，则统一通过 TrayCommand::Toggle 执行显隐切换与多重防抖
            if state_clone.borrow().is_some() {
                if let Some(ref tx) = *tray_tx_for_act.borrow() {
                    let _ = tx.send_blocking(TrayCommand::Toggle);
                }
                return;
            }

            info!("Vok GTK 应用激活初始化...");

            // 保持 GTK 守护进程在所有窗口隐藏时不退出
            let hold_guard = app.hold();

            // 0. 禁用 GTK 4 应用内部动画 (彻底消除组件级过渡、淡入淡出与缩放延迟)
            if let Some(settings) = gtk4::Settings::default() {
                settings.set_gtk_enable_animations(false);
            }

            let storage = storage_clone_for_act.clone();

            // 0.2 读取已保存的全局唤醒快捷键 (若无则默认 F12) 并绑定至 GNOME 桌面环境
            let active_shortcut = match storage.get_setting("hotkey.summon") {
                Ok(Some(s)) if !s.trim().is_empty() => s,
                _ => "F12".to_string(),
            };
            vok_infra::desktop::DesktopShortcut::bind_gnome_shortcut(&active_shortcut);
            vok_infra::desktop::DesktopShortcut::ensure_desktop_entry();

            // 0.2 启动异步缩略图后台流水线
            let thumbnail_worker = match ThumbnailWorker::start() {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    error!("启动缩略图流水线失败: {}", e);
                    panic!("Fatal: 无法启动缩略图流水线: {}", e);
                }
            };

            // 0.3 初始化主题引擎与独立设置窗口
            let theme_manager = ThemeManager::new(storage.clone());
            let settings_window = Rc::new(SettingsWindow::new(storage.clone(), theme_manager.clone()));

            // 1. 获取工作区几何（避让任务栏）
            let geom = WindowAdapter::get_work_area();
            info!("初始化全屏遮罩窗口，基准尺寸: {}", geom);

            // 2. 初始化核心会话状态机
            let state_machine = Rc::new(RefCell::new(SessionStateMachine::default()));

            // 3. 构建全屏遮罩窗口
            let overlay = Rc::new(OverlayWindow::new(app, geom, storage, thumbnail_worker));

            // 3.1 动态联动主题引擎与工作台
            {
                let overlay_clone = overlay.clone();
                theme_manager.connect_theme_changed(move |_mode, is_dark| {
                    overlay_clone.set_theme_dark(is_dark);
                });
                overlay.set_theme_dark(theme_manager.is_dark());
            }

            // 3.2 绑定工作台右上角设置按钮打开独立设置窗口
            {
                let settings_win_clone = settings_window.clone();
                overlay.set_on_open_settings(move || {
                    settings_win_clone.present();
                });
            }

            // 3.3 绑定设置窗口快捷键变更联动工作台底部提示与内核按键监听
            {
                let overlay_clone = overlay.clone();
                let evdev_clone = evdev_clone_for_act.clone();
                settings_window.set_on_hotkey_changed(move |new_config| {
                    if let Some(ref evdev) = evdev_clone {
                        evdev.update_target_hotkey(&new_config);
                    }
                    overlay_clone.update_hotkey_display(new_config);
                });
            }

            // 3.4 绑定设置窗口授权成功后即刻热挂载内核硬件级鼠标与键盘监听
            {
                let overlay_clone = overlay.clone();
                let evdev_clone = evdev_clone_for_act.clone();
                settings_window.set_on_permission_granted(move || {
                    info!("【规范授权响应】检测到系统硬件权限已授予，即刻热挂载 EvdevMouseWatcher 与 EvdevMonitor");
                    overlay_clone.ensure_evdev_watcher();
                    if let Some(ref evdev) = evdev_clone {
                        evdev.scan_now();
                    }
                });
            }

            // 4. 绑定窗口 Esc / 点击外部关闭行为到状态机
            {
                let sm_clone = state_machine.clone();
                let overlay_clone = overlay.clone();
                let last_state_change_dismiss = last_state_change.clone();
                let detector_dismiss = detector_clone_for_act.clone();
                overlay.set_on_dismiss(move || {
                    detector_dismiss.borrow_mut().cancel_all();
                    let gen = sm_clone.borrow().current_generation();
                    let _action = sm_clone.borrow_mut().handle_event(SessionEvent::EscPressed(gen));
                    overlay_clone.hide();
                    sm_clone.borrow_mut().handle_event(SessionEvent::Unmapped(gen));
                    *last_state_change_dismiss.borrow_mut() = Some(std::time::Instant::now());
                    info!("工作台已退出隐藏。");
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

            // 5.1 绑定反向拖出开始行为 (进入透明代理锚点，保持 Surface 存活，符合 R3 铁律)
            {
                let sm_clone = state_machine.clone();
                let overlay_clone = overlay.clone();
                overlay.set_on_drag_begin(move || {
                    let gen = sm_clone.borrow().current_generation();
                    let action = sm_clone.borrow_mut().handle_event(SessionEvent::DragStarted(gen));
                    if matches!(action, SessionAction::KeepProxyAnchor(_)) {
                        overlay_clone.enter_proxy_anchor_mode();
                        info!("【反向拖出】会话 [{}] 进入透明代理锚点模式", gen);
                    }
                });
            }

            // 5.2 绑定反向拖出结束/取消行为 (退出透明代理并安全隐退)
            {
                let sm_clone = state_machine.clone();
                let overlay_clone = overlay.clone();
                let last_state_change_drag = last_state_change.clone();
                let detector_drag = detector_clone_for_act.clone();
                overlay.set_on_drag_end(move || {
                    detector_drag.borrow_mut().cancel_all();
                    let gen = sm_clone.borrow().current_generation();
                    let action = sm_clone.borrow_mut().handle_event(SessionEvent::DragEnded(gen));
                    if matches!(action, SessionAction::Hide(_)) {
                        overlay_clone.hide();
                        sm_clone.borrow_mut().handle_event(SessionEvent::Unmapped(gen));
                        *last_state_change_drag.borrow_mut() = Some(std::time::Instant::now());
                        info!("【反向拖出完成】会话 [{}] 代理模式安全退出并隐退", gen);
                    }
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
            let last_state_change_for_events = last_state_change.clone();
            let detector_events = detector_clone_for_act.clone();

            glib::spawn_future_local(async move {
                while let Ok(event) = async_rx.recv().await {
                    detector_events.borrow_mut().cancel_all();
                    let mut sm = sm_for_events.borrow_mut();
                    match event.state {
                        vok_infra::input::KeyState::Pressed => {
                            let action = sm.handle_event(SessionEvent::PhysicalPress(event.timestamp));
                            if let SessionAction::Reveal(gen) = action {
                                info!("【热键按下】呈现工作台 {}", gen);
                                *last_state_change_for_events.borrow_mut() = Some(event.timestamp);
                                overlay_for_events.reveal(gen);

                                let commit_act = sm.handle_event(SessionEvent::FrameCommitted(gen));
                                if matches!(commit_act, SessionAction::Hide(_)) {
                                    overlay_for_events.hide();
                                    *last_state_change_for_events.borrow_mut() = Some(std::time::Instant::now());
                                }
                            } else if let SessionAction::Hide(gen) = action {
                                info!("【热键按下】Toggle 再次触发，隐藏工作台 {}", gen);
                                overlay_for_events.hide();
                                sm.handle_event(SessionEvent::Unmapped(gen));
                                *last_state_change_for_events.borrow_mut() = Some(event.timestamp);
                            }
                        }
                        vok_infra::input::KeyState::Released => {
                            let action = sm.handle_event(SessionEvent::PhysicalRelease(event.timestamp));
                            if let SessionAction::Hide(gen) = action {
                                info!("【热键释放】长按 Momentary 结束，隐藏工作台 {}", gen);
                                overlay_for_events.hide();
                                sm.handle_event(SessionEvent::Unmapped(gen));
                                *last_state_change_for_events.borrow_mut() = Some(event.timestamp);
                            }
                        }
                        vok_infra::input::KeyState::Repeat => {}
                    }
                }
            });

            // 8. 启动系统托盘常驻守护服务 (StatusNotifierItem)
            let (tray_tx, tray_rx) = async_channel::unbounded::<TrayCommand>();
            *tray_tx_cell.borrow_mut() = Some(tray_tx.clone());
            let tx_toggle = tray_tx.clone();
            let tx_settings = tray_tx.clone();
            let tx_quit = tray_tx;

            let overlay_for_tray = overlay.clone();
            let settings_for_tray = settings_window.clone();
            let sm_for_tray = state_machine.clone();
            let app_for_tray = app.clone();
            let last_state_change_for_tray = last_state_change.clone();
            let evdev_for_tray = evdev_clone_for_act.clone();
            let detector_tray = detector_clone_for_act.clone();

            glib::spawn_future_local(async move {
                while let Ok(cmd) = tray_rx.recv().await {
                    match cmd {
                        TrayCommand::Toggle => {
                            let now = std::time::Instant::now();

                            // 1. 硬件级长按保护：若底层物理热键正处于按下状态，说明是用户长按过程中的系统连发或瞬态回声，直接忽略
                            if evdev_for_tray.as_ref().map_or(false, |e| e.is_hotkey_active()) {
                                info!("【Toggle 过滤】物理热键当前正处于按下激活状态，忽略外部/系统重复连发");
                                continue;
                            }

                            let is_visible = overlay_for_tray.is_visible();
                            if !is_visible {
                                // 工作台当前处于隐藏状态 -> 立即呼出
                                detector_tray.borrow_mut().cancel_all();

                                // 检查双向翻转防抖 (250ms)
                                let last_change = *last_state_change_for_tray.borrow();
                                if let Some(last_change) = last_change {
                                    if now.duration_since(last_change) < std::time::Duration::from_millis(250) {
                                        info!(
                                            "【Toggle 防抖】忽略距上次状态切换仅 {}ms 的翻转指令",
                                            now.duration_since(last_change).as_millis()
                                        );
                                        continue;
                                    }
                                }

                                *last_state_change_for_tray.borrow_mut() = Some(now);
                                {
                                    let mut det = detector_tray.borrow_mut();
                                    det.reveal_instant = Some(now);
                                    det.is_holding = false;
                                }

                                let action = sm_for_tray.borrow_mut().handle_event(SessionEvent::PhysicalPress(now));
                                let _ = sm_for_tray.borrow_mut().handle_event(SessionEvent::PhysicalRelease(now));
                                if let SessionAction::Reveal(gen) = action {
                                    overlay_for_tray.reveal(gen);
                                    let commit_act = sm_for_tray.borrow_mut().handle_event(SessionEvent::FrameCommitted(gen));
                                    if matches!(commit_act, SessionAction::Hide(_)) {
                                        overlay_for_tray.hide();
                                        *last_state_change_for_tray.borrow_mut() = Some(std::time::Instant::now());
                                    }
                                    info!("【托盘/外部 Toggle】工作台已呈现 [{}]。", gen);
                                }
                            } else {
                                // 工作台当前处于可见状态
                                let mut det = detector_tray.borrow_mut();

                                // 情况 A：已经处于长按保持状态中 (连发心跳保活)
                                if det.is_holding {
                                    // 这是长按过程中的又一次 typematic 连发心跳，刷新释放检测计时器 (350ms 规避 GNOME 调度抖动)
                                    if let Some(s) = det.silence_source.take() {
                                        s.remove();
                                    }

                                    let overlay = overlay_for_tray.clone();
                                    let sm = sm_for_tray.clone();
                                    let last_state = last_state_change_for_tray.clone();
                                    let det_clone = detector_tray.clone();

                                    det.silence_source = Some(glib::timeout_add_local_once(
                                        std::time::Duration::from_millis(350),
                                        move || {
                                            info!("【长按释放感知】连发心跳停止 (>350ms)，判定长按结束松手，隐藏工作台");
                                            let mut d = det_clone.borrow_mut();
                                            d.is_holding = false;
                                            d.reveal_instant = None;
                                            d.silence_source = None;
                                            d.pending_hold_source = None;
                                            let gen = sm.borrow().current_generation();
                                            let action = sm.borrow_mut().handle_event(SessionEvent::EscPressed(gen));
                                            if matches!(action, SessionAction::Hide(_)) {
                                                overlay.hide();
                                                sm.borrow_mut().handle_event(SessionEvent::Unmapped(gen));
                                                *last_state.borrow_mut() = Some(std::time::Instant::now());
                                            }
                                        },
                                    ));
                                    continue;
                                }

                                // 情况 B：尚未处于长按保持状态中，但已经有待确认长按计时器在跑 (第二发连发到达)
                                if let Some(pending) = det.pending_hold_source.take() {
                                    // 在 85ms 内收到第二发连发！100% 判定为长按连发 (Typematic Repeat)！
                                    pending.remove();
                                    det.is_holding = true;
                                    det.reveal_instant = None;
                                    info!("【长按激活】在 85ms 内收到连续连发心跳，进入长按常驻模式 (松手即隐)");

                                    let overlay = overlay_for_tray.clone();
                                    let sm = sm_for_tray.clone();
                                    let last_state = last_state_change_for_tray.clone();
                                    let det_clone = detector_tray.clone();

                                    det.silence_source = Some(glib::timeout_add_local_once(
                                        std::time::Duration::from_millis(350),
                                        move || {
                                            info!("【长按释放感知】连发心跳停止 (>350ms)，判定长按结束松手，隐藏工作台");
                                            let mut d = det_clone.borrow_mut();
                                            d.is_holding = false;
                                            d.reveal_instant = None;
                                            d.silence_source = None;
                                            d.pending_hold_source = None;
                                            let gen = sm.borrow().current_generation();
                                            let action = sm.borrow_mut().handle_event(SessionEvent::EscPressed(gen));
                                            if matches!(action, SessionAction::Hide(_)) {
                                                overlay.hide();
                                                sm.borrow_mut().handle_event(SessionEvent::Unmapped(gen));
                                                *last_state.borrow_mut() = Some(std::time::Instant::now());
                                            }
                                        },
                                    ));
                                    continue;
                                }

                                // 情况 C：首次在可见状态下收到指令
                                // 检查是否可能为呼出后的首发连发 (距呼出 350ms ~ 850ms)
                                let elapsed_since_reveal = det.reveal_instant.map(|t| now.duration_since(t));
                                let is_potential_initial_hold = elapsed_since_reveal.map_or(false, |d| {
                                    d >= std::time::Duration::from_millis(350) && d <= std::time::Duration::from_millis(850)
                                });

                                if is_potential_initial_hold {
                                    // 这极可能是长按呼出后的首发 typematic repeat！
                                    // 启动 85ms 侦听：若 85ms 内收到第 2 发，则 100% 确认为长按保持；若无，则判定为用户单次点击关闭
                                    let overlay = overlay_for_tray.clone();
                                    let sm = sm_for_tray.clone();
                                    let last_state = last_state_change_for_tray.clone();
                                    let det_clone = detector_tray.clone();

                                    det.pending_hold_source = Some(glib::timeout_add_local_once(
                                        std::time::Duration::from_millis(85),
                                        move || {
                                            info!("【单次按键关闭】未在 85ms 内检测到连发心跳，判定为单次按键切换，执行隐藏");
                                            let mut d = det_clone.borrow_mut();
                                            d.pending_hold_source = None;
                                            d.is_holding = false;
                                            d.reveal_instant = None;
                                            let gen = sm.borrow().current_generation();
                                            let action = sm.borrow_mut().handle_event(SessionEvent::EscPressed(gen));
                                            if matches!(action, SessionAction::Hide(_)) {
                                                overlay.hide();
                                                sm.borrow_mut().handle_event(SessionEvent::Unmapped(gen));
                                                *last_state.borrow_mut() = Some(std::time::Instant::now());
                                            }
                                        },
                                    ));
                                } else {
                                    // 窗口早已处于常驻显示状态（距呼出已超 850ms，或者此前为短按打开）：
                                    // 用户再次按下快捷键 -> 立即执行 Toggle 关闭！
                                    let last_change = *last_state_change_for_tray.borrow();
                                    if let Some(last_change) = last_change {
                                        if now.duration_since(last_change) < std::time::Duration::from_millis(250) {
                                            info!(
                                                "【Toggle 防抖】忽略距上次状态切换仅 {}ms 的翻转指令",
                                                now.duration_since(last_change).as_millis()
                                            );
                                            continue;
                                        }
                                    }

                                    info!("【单次按键关闭】常驻状态下收到 Toggle 指令，立即执行隐藏");
                                    det.cancel_all();
                                    let gen = sm_for_tray.borrow().current_generation();
                                    let action = sm_for_tray.borrow_mut().handle_event(SessionEvent::EscPressed(gen));
                                    if matches!(action, SessionAction::Hide(_)) {
                                        overlay_for_tray.hide();
                                        sm_for_tray.borrow_mut().handle_event(SessionEvent::Unmapped(gen));
                                        *last_state_change_for_tray.borrow_mut() = Some(now);
                                    }
                                }
                            }
                        }
                        TrayCommand::OpenSettings => {
                            info!("【托盘操作】唤起独立首选项设置窗口");
                            settings_for_tray.present();
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
            let evdev_clone_for_action = evdev_clone_for_act.clone();
            action_toggle.connect_activate(move |_, _| {
                if evdev_clone_for_action.as_ref().map_or(false, |e| e.is_hotkey_active()) {
                    return;
                }
                info!("【D-Bus 极速唤起】收到系统快捷键/外部 Toggle 动作");
                let _ = tx_action.send_blocking(TrayCommand::Toggle);
            });
            app.add_action(&action_toggle);

            // 8.2 注册 D-Bus 原生 Action "settings"，支持外部命令行直达设置
            let action_settings = gtk4::gio::SimpleAction::new("settings", None);
            let tx_action_settings = tx_settings.clone();
            action_settings.connect_activate(move |_, _| {
                info!("【D-Bus 极速唤起】收到外部 Settings 动作");
                let _ = tx_action_settings.send_blocking(TrayCommand::OpenSettings);
            });
            app.add_action(&action_settings);

            let tray_handle = match vok_infra::VokTray::start(
                move || {
                    let _ = tx_toggle.send_blocking(TrayCommand::Toggle);
                },
                move || {
                    let _ = tx_settings.send_blocking(TrayCommand::OpenSettings);
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
                _overlay: overlay.clone(),
                _settings_window: settings_window.clone(),
                _theme_manager: theme_manager,
                _state_machine: state_machine.clone(),
                _tray: tray_handle,
                _hold_guard: hold_guard,
            });

            // 首次启动自动呼出呈现
            let now = std::time::Instant::now();
            *last_state_change.borrow_mut() = Some(now);
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
