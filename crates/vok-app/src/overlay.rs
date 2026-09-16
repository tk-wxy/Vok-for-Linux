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
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Vok Overlay")
            .decorated(false)
            .default_width(geom.width as i32)
            .default_height(geom.height as i32)
            .build();

        window.add_css_class("vok-window");

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

        // 5. 动态应用三栏宽度与卡片整除防截断高度
        // 可用高度 = 屏幕总高 - Header预估高(64) - Footer(32) - 内外间距
        let content_height = geom.height.saturating_sub(120);
        let (pane_geom, card_geom) = three_pane.apply_geometry(geom.width, content_height);
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
            .label(format!("当前工作区: {} (已避让系统任务栏)", geom))
            .css_classes(["vok-footer"])
            .margin_top(4)
            .build();
        root_box.append(&footer_label);

        window.set_child(Some(&root_box));

        // 7. 注册 Esc 快捷键关闭
        let key_controller = EventControllerKey::new();
        let on_dismiss_esc = on_dismiss.clone();
        key_controller.connect_key_pressed(move |_ctrl, keyval, _keycode, _state| {
            if keyval == Key::Escape {
                debug!("用户按下 Esc 键请求退出工作台");
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
