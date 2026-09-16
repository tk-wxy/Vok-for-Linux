use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, EventControllerKey, GestureClick, Label,
    Orientation,
};
use tracing::{debug, info};
use vok_core::generation::GenerationId;
use vok_infra::desktop::ThumbnailWorker;
use vok_infra::storage::StorageClient;
use vok_infra::window::WorkAreaGeometry;

use crate::ui::{apply_theme, HeaderComponent, ThreePaneContainer};

type DismissCallback = Box<dyn Fn()>;

/// 全屏半透明工作台主窗口
pub struct OverlayWindow {
    window: ApplicationWindow,
    header: Rc<HeaderComponent>,
    three_pane: Rc<ThreePaneContainer>,
    on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
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
                    "检测到主显示器: 逻辑尺寸 {}x{}, 缩放比={}, 原始工作区: {}",
                    rect.width(),
                    rect.height(),
                    scale,
                    geom
                );
                // 1. 获取左侧任务栏 (Dock) 与顶部状态栏的实际物理->逻辑避让偏移
                let dock_offset_x = if geom.x > 0 && scale > 1 && geom.x >= rect.x() + 30 {
                    (geom.x / scale) as u32
                } else if geom.x > 0 {
                    geom.x as u32
                } else {
                    0
                };
                let panel_offset_y = if geom.y > 0 && scale > 1 && geom.y >= rect.y() + 20 {
                    (geom.y / scale) as u32
                } else if geom.y > 0 {
                    geom.y as u32
                } else {
                    0
                };

                // 2. 扣除左侧任务栏后的净可用宽度 (严防超出屏幕右边界)
                let net_screen_w = (rect.width() as u32).saturating_sub(dock_offset_x);
                let w = if geom.width > rect.width() as u32 && scale > 1 {
                    (geom.width / scale as u32).min(net_screen_w)
                } else if geom.width > 0 && geom.width <= net_screen_w {
                    geom.width
                } else {
                    net_screen_w
                };

                let net_screen_h = (rect.height() as u32).saturating_sub(panel_offset_y);
                let h = if geom.height > rect.height() as u32 && scale > 1 {
                    (geom.height / scale as u32).min(net_screen_h)
                } else if geom.height > 0 && geom.height <= net_screen_h {
                    geom.height
                } else {
                    net_screen_h
                };

                // 3. 预留右侧与底部 16px 安全间隙，确保三栏完全居于屏幕可见区，绝不贴边或溢出
                let safe_w = w.saturating_sub(16);
                let safe_h = h.saturating_sub(12);

                info!(
                    "工作区避让计算完成: 任务栏左避让={}px, 顶避让={}px | 最终三栏可用尺寸: {}x{}",
                    dock_offset_x,
                    panel_offset_y,
                    safe_w,
                    safe_h
                );

                (safe_w, safe_h)
            } else {
                (geom.width.saturating_sub(16), geom.height.saturating_sub(12))
            }
        } else {
            (geom.width.saturating_sub(16), geom.height.saturating_sub(12))
        };

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Vok Overlay")
            .decorated(false)
            .default_width(logical_width as i32)
            .default_height(logical_height as i32)
            .build();

        // 不调用 maximize，直接使用精准计算的逻辑几何尺寸
        // window.maximize();
        window.add_css_class("vok-window");

        // 绑定 map 信号：在窗口映射时自动注入 SKIP_TASKBAR 和 UTILITY 属性
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

        let on_dismiss: Rc<RefCell<Option<DismissCallback>>> = Rc::new(RefCell::new(None));

        // 4. 水平 1:2:1 三栏 (启动台 25% : 中转站 50% : 剪贴板 25%)
        let three_pane = Rc::new(ThreePaneContainer::new(
            storage,
            thumbnail_worker,
            on_dismiss.clone(),
        ));
        root_box.append(three_pane.widget());

        // 5. 动态应用三栏宽度与卡片整除防截断高度 (基于逻辑像素计算)
        let content_height = logical_height.saturating_sub(120);
        let (pane_geom, card_geom) = three_pane.apply_geometry(logical_width, content_height);
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
                "工作区: {}x{} (已适配 HiDPI 缩放与任务栏避让) | 快捷键: F12 呼出/关闭, Esc 退出",
                logical_width, logical_height
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

        // 7. 注册 Esc / F12 快捷键关闭
        let key_controller = EventControllerKey::new();
        let on_dismiss_esc = on_dismiss.clone();
        key_controller.connect_key_pressed(move |_ctrl, keyval, _keycode, _state| {
            if keyval == Key::Escape || keyval == Key::F12 {
                debug!("用户按下 Esc/F12 键请求退出工作台");
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

        Self {
            window,
            header,
            three_pane,
            on_dismiss,
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

    /// 呈现窗口并默认聚焦于顶部搜索框
    pub fn reveal(&self, gen: GenerationId) {
        self.header.set_status(&format!("会话中 [{gen}]"));
        self.window.present();
        // 确保窗口跳过任务栏
        vok_infra::WindowAdapter::apply_skip_taskbar_by_title("Vok Overlay");
        // 呼出即入焦：自动聚焦于搜索框
        self.header.search_entry().grab_focus();
    }

    /// 隐藏窗口
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// 是否处于可见状态
    #[allow(dead_code)]
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }
}
