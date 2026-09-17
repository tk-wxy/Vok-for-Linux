use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::gdk::Key;
use gtk4::glib::translate::IntoGlib;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, EventControllerKey, GestureClick, Label,
    Orientation,
};
use tracing::{debug, info};
use vok_core::generation::GenerationId;
use vok_core::hotkey::HotkeyConfig;
use vok_infra::desktop::ThumbnailWorker;
use vok_infra::storage::StorageClient;
use vok_infra::window::WorkAreaGeometry;

use crate::ui::{apply_theme, HeaderComponent, SettingsPopover, ThreePaneContainer};

type DismissCallback = Box<dyn Fn()>;

/// 全屏半透明工作台主窗口
pub struct OverlayWindow {
    window: ApplicationWindow,
    header: Rc<HeaderComponent>,
    three_pane: Rc<ThreePaneContainer>,
    _settings_popover: Rc<SettingsPopover>,
    on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
    previous_active_window: Rc<Cell<Option<u32>>>,
    has_been_active: Rc<Cell<bool>>,
}

impl OverlayWindow {
    /// 构建全屏避让任务栏的三栏工作台主窗口
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
                let scale = monitor.scale_factor().max(1);
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

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Vok Overlay")
            .decorated(false)
            .default_width(logical_width as i32)
            .default_height(logical_height as i32)
            .build();

        // 窗口最大化避让任务栏 (在 GNOME/Mutter 下首帧前自动严密贴合 _NET_WORKAREA，杜绝居中跳动与缩放抽搐)
        window.maximize();
        window.add_css_class("vok-window");

        // 绑定 map 信号：在窗口每次映射时确保权威注入 SKIP_TASKBAR, ABOVE 和 MAXIMIZED
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

        // 3.1 挂载设置 Popover 面板
        let settings_popover = Rc::new(SettingsPopover::new(storage.clone(), header.settings_btn()));
        {
            let popover_clone = settings_popover.clone();
            header.settings_btn().connect_clicked(move |_| {
                popover_clone.popup();
            });
        }

        let on_dismiss: Rc<RefCell<Option<DismissCallback>>> = Rc::new(RefCell::new(None));

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

        // 联动快捷键变更：实时刷新 footer 提示与当前比对按键
        {
            let footer_label_clone = footer_label.clone();
            let current_hotkey_clone = current_hotkey.clone();
            settings_popover.set_on_hotkey_changed(move |new_config| {
                let display = new_config.to_display_string();
                footer_label_clone.set_label(&format!(
                    "工作区: {}x{} (已适配 HiDPI 缩放与任务栏避让) | 快捷键: {} 呼出/关闭, Esc 退出",
                    logical_width, logical_height, display
                ));
                *current_hotkey_clone.borrow_mut() = new_config;
            });
        }

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
        let on_dismiss_esc = on_dismiss.clone();
        let current_hotkey_for_key = current_hotkey.clone();
        key_controller.connect_key_pressed(move |_ctrl, keyval, _keycode, modifier_state| {
            if keyval == Key::Escape {
                debug!("用户按下 Esc 键请求退出工作台");
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

        // 8. 点击背景空白处关闭
        let click_gesture = GestureClick::new();
        let on_dismiss_click = on_dismiss.clone();
        click_gesture.connect_released(move |_gesture, _n_press, _x, _y| {
            debug!("用户点击工作台空白背景请求关闭");
            if let Some(cb) = on_dismiss_click.borrow().as_ref() {
                cb();
            }
        });
        root_box.add_controller(click_gesture);

        // 9. 失焦自动退出 (Focus-Out Auto-Close)
        let has_been_active = Rc::new(Cell::new(false));
        let previous_active_window = Rc::new(Cell::new(None));
        {
            let on_dismiss_focus = on_dismiss.clone();
            let has_been_active_clone = has_been_active.clone();
            window.connect_is_active_notify(move |win| {
                let active = win.is_active();
                debug!("OverlayWindow is_active 状态变更: {}", active);
                if active {
                    has_been_active_clone.set(true);
                } else if has_been_active_clone.get() && win.is_visible() {
                    has_been_active_clone.set(false);
                    info!("【失焦感知】OverlayWindow 失去前台活动焦点 (Focus-Out)，触发自动退出");
                    if let Some(cb) = on_dismiss_focus.borrow().as_ref() {
                        cb();
                    }
                }
            });
        }

        Self {
            window,
            header,
            three_pane,
            _settings_popover: settings_popover,
            on_dismiss,
            previous_active_window,
            has_been_active,
        }
    }

    /// 设置工作台关闭回调 (Esc / 点击空白处)
    pub fn set_on_dismiss<F: Fn() + 'static>(&self, callback: F) {
        *self.on_dismiss.borrow_mut() = Some(Box::new(callback));
    }

    /// 设置中转站外部拖拽丢入完成回调
    pub fn set_on_drop<F: Fn(Vec<PathBuf>) + 'static>(&self, callback: F) {
        self.three_pane.set_on_drop(callback);
    }

    /// 呈现窗口并默认聚焦于顶部搜索框，同时暂存原前台活动窗口
    pub fn reveal(&self, gen: GenerationId) {
        // 呼出前暂存当前活动窗口 (原前台应用)
        if let Some(xid) = vok_infra::WindowAdapter::get_active_window_id() {
            let is_self = vok_infra::WindowAdapter::get_window_title(xid)
                .is_some_and(|t| t == "Vok Overlay");
            if !is_self {
                let title = vok_infra::WindowAdapter::get_window_title(xid)
                    .unwrap_or_else(|| "未知应用".to_string());
                info!("【前台焦点暂存】捕获到前台活动窗口: [0x{:x}] ({})", xid, title);
                self.previous_active_window.set(Some(xid));
            }
        }

        self.has_been_active.set(false);
        self.header.set_status(&format!("会话中 [{gen}]"));
        self.window.maximize();
        self.window.set_visible(true);
        self.window.present();

        // 呼出即入焦：自动聚焦于搜索框
        self.header.search_entry().grab_focus();
    }

    /// 隐藏窗口并原子交还原前台窗口焦点
    pub fn hide(&self) {
        self.has_been_active.set(false);
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
