use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::gdk::Key;
use gtk4::glib::translate::IntoGlib;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, EventControllerKey,
    Label, Orientation,
};

use tracing::{debug, info, warn};

use vok_core::generation::GenerationId;
use vok_core::hotkey::HotkeyConfig;
use vok_infra::desktop::ThumbnailWorker;
use vok_infra::storage::StorageClient;
use vok_infra::window::WorkAreaGeometry;

use crate::ui::{apply_theme, HeaderComponent, ThreePaneContainer};

type DismissCallback = Box<dyn Fn()>;
type OpenSettingsCallback = Box<dyn Fn()>;

/// 全屏半透明工作台主窗口
pub struct OverlayWindow {
    window: ApplicationWindow,
    root_box: GtkBox,
    header: Rc<HeaderComponent>,
    three_pane: Rc<ThreePaneContainer>,
    footer_label: Label,
    geom: Rc<Cell<WorkAreaGeometry>>,
    current_hotkey: Rc<RefCell<HotkeyConfig>>,
    on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
    on_open_settings: Rc<RefCell<Option<OpenSettingsCallback>>>,
    previous_active_window: Rc<Cell<Option<u32>>>,
    reveal_time: Rc<Cell<std::time::Instant>>,
    is_in_proxy_mode: Rc<Cell<bool>>,
    has_gained_focus: Rc<Cell<bool>>,
    outside_click_watcher: Option<Rc<vok_infra::OutsideClickWatcher>>,
    outside_tx: async_channel::Sender<()>,
    evdev_mouse_watcher: RefCell<Option<Rc<vok_infra::EvdevMouseWatcher>>>,
}


impl OverlayWindow {
    /// 构建全屏半透明工作台并初始化子组件
    pub fn new(
        app: &Application,
        geom: WorkAreaGeometry,
        storage: StorageClient,
        thumbnail_worker: Arc<ThumbnailWorker>,
    ) -> Self {
        // 获取当前主显示器的逻辑几何尺寸与 HiDPI 缩放比
        let (logical_width, logical_height) = if let Some(display) = gtk4::gdk::Display::default() {
            let monitors = display.monitors();
            if let Some(monitor) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() {
                let rect = monitor.geometry();
                let scale = monitor.scale_factor();
                info!(
                    "检测到主显示器: 逻辑矩形 {}x{}+{}+{}, 缩放比={}, 原始系统工作区: {}",
                    rect.width(),
                    rect.height(),
                    rect.x(),
                    rect.y(),
                    scale,
                    geom
                );

                // 物理工作区 -> 逻辑工作区动态无损映射
                // 任何桌面环境下，_NET_WORKAREA 均已权威剔除所有位置（左/右/顶/底）的任务栏与面板
                let (lw, lh) = if geom.width > rect.width() as u32 && scale > 1 {
                    let w = (geom.width / scale as u32).min(rect.width() as u32);
                    let h = (geom.height / scale as u32).min(rect.height() as u32);
                    (w, h)
                } else if geom.width > 0 && geom.height > 0 {
                    let w = geom.width.min(rect.width() as u32);
                    let h = geom.height.min(rect.height() as u32);
                    (w, h)
                } else {
                    (rect.width() as u32, rect.height() as u32)
                };

                info!(
                    "通用全屏工作区适配就绪: 逻辑尺寸 {}x{} (100% 严丝合缝全屏覆盖，零缝隙，零圆角)",
                    lw, lh
                );
                (lw, lh)
            } else {
                (geom.width, geom.height)
            }
        } else {
            (geom.width, geom.height)
        };

        let geom = Rc::new(Cell::new(geom));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Vok Overlay")
            .decorated(false)
            .visible(false)
            .default_width(logical_width as i32)
            .default_height(logical_height as i32)
            .build();

        // 窗口最大化避让任务栏 (在 GNOME/Mutter 下首帧前自动严密贴合 _NET_WORKAREA，杜绝居中跳动与缩放抽搐)
        window.maximize();
        window.add_css_class("vok-window");

        // 绑定 map 信号：在窗口每次映射时确保权威注入 SKIP_TASKBAR 和 MAXIMIZED
        window.connect_map(|_| {
            vok_infra::WindowAdapter::apply_skip_taskbar_by_title("Vok Overlay");
        });

        // 1. 加载全局深色毛玻璃 CSS 主题
        apply_theme();

        // 2. 根纵向容器
        let root_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .css_classes(["vok-root-container"])
            .hexpand(true)
            .vexpand(true)
            .spacing(12)
            .build();

        // 3. 顶部 Header 区 (Logo + 全局搜索框 + 设置按钮)
        let header = Rc::new(HeaderComponent::new());
        root_box.append(header.widget());

        // 读取持久化快捷键配置 (默认 F12)
        let initial_hotkey = match storage.get_setting("hotkey.summon") {
            Ok(Some(s)) if !s.trim().is_empty() => {
                HotkeyConfig::from_gsettings_string(&s).unwrap_or_else(|_| HotkeyConfig::default_summon())
            }
            _ => HotkeyConfig::default_summon(),
        };
        let current_hotkey = Rc::new(RefCell::new(initial_hotkey));

        let on_dismiss: Rc<RefCell<Option<DismissCallback>>> = Rc::new(RefCell::new(None));
        let on_open_settings: Rc<RefCell<Option<OpenSettingsCallback>>> = Rc::new(RefCell::new(None));

        // 3.1 挂载右上角设置齿轮按钮：隐退工作台并唤起独立首选项窗口
        {
            let on_dismiss_for_settings = on_dismiss.clone();
            let on_open_settings_clone = on_open_settings.clone();
            header.settings_btn().connect_clicked(move |_| {
                info!("【设置入口】用户点击 Header 设置按钮，隐退工作台并唤起独立首选项窗口");
                if let Some(cb) = on_dismiss_for_settings.borrow().as_ref() {
                    cb();
                }
                if let Some(cb) = on_open_settings_clone.borrow().as_ref() {
                    cb();
                }
            });
        }

        // 4. 水平 1:2:1 三栏 (启动台 25% : 中转站 50% : 剪贴板 25%)
        let three_pane = Rc::new(ThreePaneContainer::new(
            storage,
            thumbnail_worker,
            on_dismiss.clone(),
        ));
        root_box.append(three_pane.widget());

        // 5. 动态应用三栏宽度与卡片整除防截断高度 (基于逻辑像素计算)
        // 容器内边距: padding: 16px 20px (左右内边距共 40px, 纵向内边距共 32px)
        let content_width = logical_width.saturating_sub(40);
        // 纵向消耗: padding_y (32px) + header (~54px) + footer (~24px) + spacing (12*2=24px) ≈ 134px
        let content_height = logical_height.saturating_sub(134);
        let (pane_geom, card_geom) = three_pane.apply_geometry(content_width, content_height);
        info!(
            "三栏几何已计算: 左={}px, 中={}px, 右={}px | 卡片整除行数={}, 单行高={}px (100%防截断)",
            pane_geom.left_width,
            pane_geom.middle_width,
            pane_geom.right_width,
            card_geom.row_count,
            card_geom.row_height
        );

        // 6. 底部状态与工作区提示
        let footer_label = Label::builder()
            .label(format!(
                "工作区: {}x{} (已适配 HiDPI 缩放与任务栏避让) | 快捷键: {} 呼出/关闭, Esc 退出",
                logical_width, logical_height, current_hotkey.borrow().to_display_string()
            ))
            .css_classes(["vok-footer"])
            .margin_top(4)
            .build();
        root_box.append(&footer_label);

        window.set_child(Some(&root_box));

        three_pane.debug_sizes();
        let (w_header, _) = header.widget().preferred_size();
        let (w_pane, _) = three_pane.widget().preferred_size();
        let (w_root, _) = root_box.preferred_size();
        info!("【尺寸排查】header min={} nat={}, three_pane min={} nat={}, root min={} nat={}",
            w_header.width(), w_header.width(),
            w_pane.width(), w_pane.width(),
            w_root.width(), w_root.width()
        );

        // 7. 注册 Esc / 动态快捷键关闭
        let key_controller = EventControllerKey::new();
        // 核心加固：设置为 Capture 阶段，抢在 SearchEntry 等子控件前拦截 Escape 键
        key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let on_dismiss_esc = on_dismiss.clone();
        let current_hotkey_for_key = current_hotkey.clone();
        key_controller.connect_key_pressed(move |_ctrl, keyval, _keycode, modifier_state| {
            if keyval == Key::Escape {
                debug!("【Esc 键拦截】用户按下 Esc 键请求退出工作台");
                if let Some(cb) = on_dismiss_esc.borrow().as_ref() {
                    cb();
                }
                return glib::Propagation::Stop;
            }

            // 动态比对当前生效的唤醒快捷键 (实现窗口内再次按下唤醒键执行 Toggle 关闭)
            let cur = current_hotkey_for_key.borrow();
            let is_match = {
                let ctrl = modifier_state.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
                let alt = modifier_state.contains(gtk4::gdk::ModifierType::ALT_MASK);
                let shift = modifier_state.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                let super_key = modifier_state.contains(gtk4::gdk::ModifierType::SUPER_MASK);

                let mask_match = cur.modifiers.ctrl == ctrl
                    && cur.modifiers.alt == alt
                    && cur.modifiers.shift == shift
                    && cur.modifiers.super_key == super_key;

                let key_match = if cur.keyval > 0 && cur.keyval == keyval.into_glib() {
                    true
                } else if let Some(name) = keyval.name() {
                    name.eq_ignore_ascii_case(&cur.key_name)
                } else {
                    false
                };

                mask_match && key_match
            };

            if is_match {
                debug!("用户在窗口内再次按下唤醒快捷键，执行 Toggle 退出");
                if let Some(cb) = on_dismiss_esc.borrow().as_ref() {
                    cb();
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(key_controller);

        // 7.1 搜索框 stop-search 兜底拦截
        {
            let on_dismiss_search = on_dismiss.clone();
            header.search_entry().connect_stop_search(move |_| {
                debug!("【搜索框】触发 stop-search，退出工作台");
                if let Some(cb) = on_dismiss_search.borrow().as_ref() {
                    cb();
                }
            });
        }

        let is_in_proxy_mode = Rc::new(Cell::new(false));
        let reveal_time = Rc::new(Cell::new(std::time::Instant::now()));
        let previous_active_window = Rc::new(Cell::new(None));

        // 8. 全局工作区外物理点击监听 (基于 XInput2 RawButtonPress，无侵入，不独占抓取，零按键干扰，零鼠标晃动误触发)
        let (outside_tx, outside_rx) = async_channel::unbounded::<()>();
        {
            let on_dismiss_outside = on_dismiss.clone();
            let is_proxy_outside = is_in_proxy_mode.clone();
            let reveal_time_outside = reveal_time.clone();
            let win_weak = window.downgrade();
            glib::spawn_future_local(async move {
                while let Ok(()) = outside_rx.recv().await {
                    if is_proxy_outside.get() {
                        continue;
                    }
                    if reveal_time_outside.get().elapsed().as_millis() < 200 {
                        continue;
                    }
                    let Some(win) = win_weak.upgrade() else {
                        break;
                    };
                    if !win.is_visible() {
                        continue;
                    }
                    info!("【外置点击隐退】收到外部物理点击事件，执行工作台自动隐藏");
                    if let Some(cb) = on_dismiss_outside.borrow().as_ref() {
                        cb();
                    }
                }
            });
        }

        let outside_tx_xi2 = outside_tx.clone();
        let outside_click_watcher = match vok_infra::OutsideClickWatcher::start(geom.get(), move || {
            let _ = outside_tx_xi2.send_blocking(());
        }) {
            Ok(w) => Some(Rc::new(w)),
            Err(e) => {
                warn!("启动 OutsideClickWatcher 失败: {:?}", e);
                None
            }
        };

        let outside_tx_evdev = outside_tx.clone();
        let initial_vok_xid = vok_infra::WindowAdapter::get_vok_window_id().unwrap_or(0);
        let evdev_mouse_watcher = RefCell::new(match vok_infra::EvdevMouseWatcher::start(geom.get(), initial_vok_xid, move || {
            let _ = outside_tx_evdev.try_send(());
        }) {
            Ok(w) => {
                info!("【内核硬件鼠标监听就绪】成功启动 EvdevMouseWatcher，全屏盲区物理点击零延迟响应");
                Some(Rc::new(w))
            }
            Err(e) => {
                debug!("未能启动 EvdevMouseWatcher (原因: {:?})，可前往首选项进行规范授权", e);
                None
            }
        });



        // 9. 失焦自动退出 (Focus-Out Auto-Close)
        let has_gained_focus = Rc::new(Cell::new(false));

        // 9.1 挂载窗口 is_active 变更监听 (切换至其它应用时)
        {
            let on_dismiss_focus = on_dismiss.clone();
            let is_proxy_for_focus = is_in_proxy_mode.clone();
            let reveal_time_focus = reveal_time.clone();
            let has_gained_focus_notify = has_gained_focus.clone();
            window.connect_is_active_notify(move |win| {
                let active = win.is_active();
                debug!("OverlayWindow is_active 状态变更: {}", active);
                if active {
                    has_gained_focus_notify.set(true);
                    info!("【入焦成功】OverlayWindow 已获得前台活动焦点");
                } else if has_gained_focus_notify.get() && win.is_visible() {
                    // 规则 R3 守卫：处于反向拖出透明代理锚点模式时，指针移动至外部窗口引起的失焦绝对不能触发退出
                    if is_proxy_for_focus.get() {
                        debug!("【失焦守卫】当前正处于反向拖出透明代理锚点模式 (R3)，忽略失焦通知");
                        return;
                    }
                    if reveal_time_focus.get().elapsed().as_millis() < 250 {
                        return;
                    }
                    info!("【失焦感知】OverlayWindow 失去前台活动焦点 (Focus-Out)，触发自动退出");
                    if let Some(cb) = on_dismiss_focus.borrow().as_ref() {
                        cb();
                    }
                }
            });
        }

        // 9.2 毫秒级兜底检测：感知 GNOME 概览、失焦与其它应用激活
        {
            let win_weak = window.downgrade();
            let is_proxy_timer = is_in_proxy_mode.clone();
            let on_dismiss_timer = on_dismiss.clone();
            let reveal_time_timer = reveal_time.clone();
            let my_xid_cell = Rc::new(Cell::new(None));
            let has_gained_focus_timer = has_gained_focus.clone();
            let geom_timer = geom.clone();

            glib::timeout_add_local(std::time::Duration::from_millis(30), move || {
                let Some(win) = win_weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if !win.is_visible() {
                    my_xid_cell.set(None);
                    return glib::ControlFlow::Continue;
                }
                if is_proxy_timer.get() {
                    return glib::ControlFlow::Continue;
                }
                let elapsed_ms = reveal_time_timer.get().elapsed().as_millis();
                if elapsed_ms < 250 {
                    return glib::ControlFlow::Continue;
                }

                // 检查 0: GNOME Shell Overview (如按下 Super / 开始键呼出系统全览)
                if vok_infra::WindowAdapter::is_gnome_overview_active() {
                    info!("【失焦感知】检测到系统进入 GNOME Shell 概览 (OverviewActive == true)，触发自动退出");
                    if let Some(cb) = on_dismiss_timer.borrow().as_ref() {
                        cb();
                    }
                    return glib::ControlFlow::Continue;
                }

                // 检查 1: GTK 自身 is_active 状态 (仅在曾经成功入焦后生效)
                if has_gained_focus_timer.get() && !win.is_active() {
                    info!("【失焦感知】轮询检测到窗口失去活动焦点 (GTK is_active == false)，触发自动退出");
                    if let Some(cb) = on_dismiss_timer.borrow().as_ref() {
                        cb();
                    }
                    return glib::ControlFlow::Continue;
                }

                // 检查 2: 前台活动窗口追踪 (Alt+Tab, Super键, 点击任务栏唤起软件, 呼出其它应用)
                if my_xid_cell.get().is_none() {
                    if let Some(id) = vok_infra::WindowAdapter::get_vok_window_id() {
                        my_xid_cell.set(Some(id));
                    }
                }
                if let Some(my_xid) = my_xid_cell.get() {
                    let active_opt = vok_infra::WindowAdapter::get_active_window_id();
                    if active_opt == Some(my_xid) {
                        // 确凿观测到本窗口已是前台活动窗口
                        has_gained_focus_timer.set(true);
                    } else if has_gained_focus_timer.get() {
                        // 确凿观测到曾经入焦，但现在前台活动窗口已变为其它窗口或桌面 (active_opt != Some(my_xid))
                        let title = active_opt
                            .and_then(vok_infra::WindowAdapter::get_window_title)
                            .unwrap_or_else(|| "桌面/Wayland应用/GNOME Shell".to_string());
                        info!("【失焦感知】检测到前台活动窗口已切换至 [{}] (!= 本窗口 0x{:x})，触发自动退出", title, my_xid);
                        if let Some(cb) = on_dismiss_timer.borrow().as_ref() {
                            cb();
                        }
                        return glib::ControlFlow::Continue;
                    }
                }

                // 检查 3: 工作区外部物理按键按下检测 (检测到任何外置硬件按键即刻隐退)
                let current_wa = geom_timer.get();
                if vok_infra::WindowAdapter::is_pointer_clicking_outside(current_wa) {
                    info!("【外置点击隐退】检测到工作区外部物理按键按下，触发自动隐藏");
                    if let Some(cb) = on_dismiss_timer.borrow().as_ref() {
                        cb();
                    }
                    return glib::ControlFlow::Continue;
                }

                glib::ControlFlow::Continue
            });
        }

        // 9.3 订阅 GNOME Shell Overview 概览信号 (按下 Super 键时 0ms 瞬时响应)
        if let Ok(dbus_conn) = gtk4::gio::bus_get_sync(gtk4::gio::BusType::Session, gtk4::gio::Cancellable::NONE) {
            let on_dismiss_dbus = on_dismiss.clone();
            let is_proxy_dbus = is_in_proxy_mode.clone();
            let win_weak_dbus = window.downgrade();
            dbus_conn.signal_subscribe(
                Some("org.gnome.Shell"),
                Some("org.freedesktop.DBus.Properties"),
                Some("PropertiesChanged"),
                Some("/org/gnome/Shell"),
                None,
                gtk4::gio::DBusSignalFlags::NONE,
                move |_conn, _sender, _path, _iface, _signal, _params| {
                    if is_proxy_dbus.get() {
                        return;
                    }
                    let Some(win) = win_weak_dbus.upgrade() else {
                        return;
                    };
                    if !win.is_visible() {
                        return;
                    }
                    if vok_infra::WindowAdapter::is_gnome_overview_active() {
                        info!("【D-Bus 概览感知】收到 org.gnome.Shell 概览激活信号，瞬时触发自动退出");
                        if let Some(cb) = on_dismiss_dbus.borrow().as_ref() {
                            cb();
                        }
                    }
                },
            );
        }

        Self {
            window,
            root_box,
            header,
            three_pane,
            footer_label,
            geom,
            current_hotkey,
            on_dismiss,
            on_open_settings,
            previous_active_window,
            reveal_time,
            is_in_proxy_mode,
            has_gained_focus,
            outside_click_watcher,
            outside_tx,
            evdev_mouse_watcher,
        }
    }

    /// 确保内核硬件级鼠标点击监听已就绪 (若先前未授权但现在已就绪，立即动态挂载)
    pub fn ensure_evdev_watcher(&self) {
        if self.evdev_mouse_watcher.borrow().is_some() {
            return;
        }
        if vok_infra::SystemPermission::has_input_permissions() {
            let outside_tx_evdev = self.outside_tx.clone();
            let current_geom = self.geom.get();
            let vok_xid = vok_infra::WindowAdapter::get_vok_window_id().unwrap_or(0);
            match vok_infra::EvdevMouseWatcher::start(current_geom, vok_xid, move || {
                let _ = outside_tx_evdev.try_send(());
            }) {
                Ok(w) => {
                    info!("【内核硬件鼠标监听就绪】动态接入 EvdevMouseWatcher，全屏盲区物理点击零延迟响应");
                    *self.evdev_mouse_watcher.borrow_mut() = Some(Rc::new(w));
                }
                Err(e) => {
                    warn!("动态启动 EvdevMouseWatcher 失败: {:?}", e);
                }
            }
        }
    }


    /// 设置工作台关闭回调 (Esc / 点击空白处)
    pub fn set_on_dismiss<F: Fn() + 'static>(&self, callback: F) {
        *self.on_dismiss.borrow_mut() = Some(Box::new(callback));
    }

    /// 设置打开设置窗口回调
    pub fn set_on_open_settings<F: Fn() + 'static>(&self, callback: F) {
        *self.on_open_settings.borrow_mut() = Some(Box::new(callback));
    }

    /// 动态刷新深浅色主题样式类
    pub fn set_theme_dark(&self, is_dark: bool) {
        if is_dark {
            self.window.remove_css_class("theme-light");
            self.root_box.remove_css_class("theme-light");
        } else {
            self.window.add_css_class("theme-light");
            self.root_box.add_css_class("theme-light");
        }
    }

    /// 动态刷新底部快捷键提示与窗口内比对键
    pub fn update_hotkey_display(&self, new_config: HotkeyConfig) {
        let display = new_config.to_display_string();
        let geom = self.geom.get();
        self.footer_label.set_label(&format!(
            "工作区: {}x{} (已适配 HiDPI 缩放与任务栏避让) | 快捷键: {} 呼出/关闭, Esc 退出",
            geom.width, geom.height, display
        ));
        *self.current_hotkey.borrow_mut() = new_config;
    }

    /// 设置中转站外部拖拽丢入完成回调
    pub fn set_on_drop<F: Fn(Vec<PathBuf>) + 'static>(&self, callback: F) {
        self.three_pane.set_on_drop(callback);
    }

    /// 设置反向拖出起手回调 (进入透明代理锚点)
    pub fn set_on_drag_begin<F: Fn() + 'static>(&self, callback: F) {
        self.three_pane.set_on_drag_begin(callback);
    }

    /// 设置反向拖出结束/取消回调 (退出透明代理并安全隐退)
    pub fn set_on_drag_end<F: Fn() + 'static>(&self, callback: F) {
        self.three_pane.set_on_drag_end(callback);
    }

    /// 进入透明代理锚点模式 (规则 R3 / 切片 2.3)
    /// 保持 Surface 存活以维系 Wayland DnD 拖拽会话，同时全透明穿透视觉
    pub fn enter_proxy_anchor_mode(&self) {
        if self.is_in_proxy_mode.get() {
            return;
        }
        info!("【透明代理锚点】激活 vok-proxy-anchor 模式，Surface 保持存活，界面全透明穿透 (规则 R3)");
        self.is_in_proxy_mode.set(true);
        if let Some(watcher) = self.outside_click_watcher.as_ref() {
            watcher.set_enabled(false);
        }
        if let Some(watcher) = self.evdev_mouse_watcher.borrow().as_ref() {
            watcher.set_enabled(false);
        }
        self.window.add_css_class("vok-proxy-anchor");
        self.root_box.add_css_class("vok-proxy-anchor");
        self.header.widget().set_visible(false);
        self.three_pane.widget().set_visible(false);
        self.footer_label.set_visible(false);
    }

    /// 退出透明代理锚点模式并恢复界面完整形态
    pub fn exit_proxy_anchor_mode(&self) {
        if !self.is_in_proxy_mode.get() {
            return;
        }
        info!("【透明代理锚点】退出 vok-proxy-anchor 模式，恢复标准视觉形态");
        self.is_in_proxy_mode.set(false);
        if self.window.is_visible() {
            if let Some(watcher) = self.outside_click_watcher.as_ref() {
                watcher.set_enabled(true);
            }
            if let Some(watcher) = self.evdev_mouse_watcher.borrow().as_ref() {
                watcher.set_enabled(true);
            }
        }
        self.window.remove_css_class("vok-proxy-anchor");
        self.root_box.remove_css_class("vok-proxy-anchor");
        self.header.widget().set_visible(true);
        self.three_pane.widget().set_visible(true);
        self.footer_label.set_visible(true);
    }

    /// 当前是否处于透明代理锚点模式
    pub fn is_in_proxy_mode(&self) -> bool {
        self.is_in_proxy_mode.get()
    }

    /// 呈现窗口并默认聚焦于顶部搜索框，同时暂存原前台活动窗口
    pub fn reveal(&self, gen: GenerationId) {
        self.exit_proxy_anchor_mode();
        self.reveal_time.set(std::time::Instant::now());
        self.has_gained_focus.set(false);

        // 1. 动态刷新最新的真实系统工作区几何
        let current_geom = vok_infra::WindowAdapter::get_work_area();
        self.geom.set(current_geom);

        // 动态检测并确保 Evdev 硬件鼠标监听已就绪 (若系统权限已变更，即刻热挂载)
        self.ensure_evdev_watcher();

        // 刷新并激活外置点击探测器
        if let Some(watcher) = self.outside_click_watcher.as_ref() {
            watcher.update_workarea(current_geom);
            let watcher_enable = watcher.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                watcher_enable.set_enabled(true);
            });
        }
        if let Some(watcher) = self.evdev_mouse_watcher.borrow().as_ref() {
            watcher.update_workarea(current_geom);
            if let Some(xid) = vok_infra::WindowAdapter::get_vok_window_id() {
                watcher.update_vok_xid(xid);
            }
            let watcher_enable = watcher.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                if let Some(xid) = vok_infra::WindowAdapter::get_vok_window_id() {
                    watcher_enable.update_vok_xid(xid);
                }
                watcher_enable.set_enabled(true);
            });
        }

        // 获取主显示器几何与 HiDPI 缩放比，重新应用自适应布局
        let (logical_width, logical_height) = if let Some(display) = gtk4::gdk::Display::default() {
            let monitors = display.monitors();
            if let Some(monitor) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() {
                let rect = monitor.geometry();
                let scale = monitor.scale_factor();
                if current_geom.width > rect.width() as u32 && scale > 1 {
                    let w = (current_geom.width / scale as u32).min(rect.width() as u32);
                    let h = (current_geom.height / scale as u32).min(rect.height() as u32);
                    (w, h)
                } else if current_geom.width > 0 && current_geom.height > 0 {
                    let w = current_geom.width.min(rect.width() as u32);
                    let h = current_geom.height.min(rect.height() as u32);
                    (w, h)
                } else {
                    (rect.width() as u32, rect.height() as u32)
                }
            } else {
                (current_geom.width, current_geom.height)
            }
        } else {
            (current_geom.width, current_geom.height)
        };

        // 2. 动态重新计算三栏几何，确保 100% 严丝合缝
        let content_width = logical_width.saturating_sub(40);
        let content_height = logical_height.saturating_sub(134);
        let (pane_geom, card_geom) = self.three_pane.apply_geometry(content_width, content_height);
        debug!(
            "【全屏工作区严合】逻辑尺寸: {}x{} | 三栏左={}px 中={}px 右={}px | 卡片行数={} 单行高={}px",
            logical_width, logical_height,
            pane_geom.left_width, pane_geom.middle_width, pane_geom.right_width,
            card_geom.row_count, card_geom.row_height
        );

        // 3. 呼出前暂存当前活动窗口 (原前台应用)
        if let Some(xid) = vok_infra::WindowAdapter::get_active_window_id() {
            let is_self = vok_infra::WindowAdapter::get_window_title(xid)
                .is_some_and(|t| t == "Vok Overlay");
            if !is_self && xid != 0 {
                let title = vok_infra::WindowAdapter::get_window_title(xid)
                    .unwrap_or_else(|| "未知应用".to_string());
                info!("【前台焦点暂存】捕获到前台活动窗口: [0x{:x}] ({})", xid, title);
                self.previous_active_window.set(Some(xid));
            }
        }

        self.header.set_status(&format!("会话中 [{gen}]"));
        self.window.maximize();
        self.window.set_visible(true);
        self.window.present();

        // 呼出提升至前台并激活
        vok_infra::WindowAdapter::raise_and_activate_window_by_title("Vok Overlay");

        // 呼出即入焦：自动聚焦于搜索框
        gtk4::prelude::GtkWindowExt::set_focus(&self.window, Some(self.header.search_entry()));
        self.header.search_entry().grab_focus();
    }

    /// 隐藏窗口并原子交还原前台窗口焦点
    pub fn hide(&self) {
        self.exit_proxy_anchor_mode();
        self.has_gained_focus.set(false);
        if let Some(watcher) = self.outside_click_watcher.as_ref() {
            watcher.set_enabled(false);
        }
        if let Some(watcher) = self.evdev_mouse_watcher.borrow().as_ref() {
            watcher.set_enabled(false);
        }
        self.window.set_visible(false);

        // 原子交还原前台焦点
        if let Some(xid) = self.previous_active_window.take() {
            if let Err(e) = vok_infra::WindowAdapter::restore_active_window(xid) {
                tracing::warn!("恢复原前台窗口 [0x{:x}] 焦点失败: {:?}", xid, e);
            }
        }
    }


    /// 获取当前暂存的原前台窗口 XID (若存在)
    pub fn previous_active_window(&self) -> Option<u32> {
        self.previous_active_window.get()
    }

    /// 是否处于可见状态
    #[allow(dead_code)]
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }
}
