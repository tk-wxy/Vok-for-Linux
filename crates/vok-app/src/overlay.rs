use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gtk4::gdk::{self, DragAction, FileList, Key};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, DropTarget, EventControllerKey,
    GestureClick, Label, Orientation,
};
use tracing::{debug, info, warn};
use vok_core::generation::GenerationId;
use vok_infra::window::WorkAreaGeometry;

pub struct OverlayWindow {
    window: ApplicationWindow,
    status_label: Label,
    #[allow(dead_code)]
    dropped_files_label: Label,
    on_dismiss: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_drop: Rc<RefCell<Option<Box<dyn Fn(Vec<PathBuf>)>>>>,
}

impl OverlayWindow {
    /// 构建全屏避让任务栏的半透明遮罩窗口
    pub fn new(app: &Application, geom: WorkAreaGeometry) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Vok Overlay")
            .decorated(false)
            .default_width(geom.width as i32)
            .default_height(geom.height as i32)
            .build();

        // 绑定 CSS 样式
        Self::apply_styles(&window);

        // 主容器
        let root_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .css_classes(["vok-root-container"])
            .hexpand(true)
            .vexpand(true)
            .build();

        // 顶部信息条
        let header_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["vok-header"])
            .spacing(12)
            .margin_top(24)
            .margin_start(32)
            .margin_end(32)
            .build();

        let title_label = Label::builder()
            .label("VOK 工作台")
            .css_classes(["vok-title"])
            .halign(Align::Start)
            .build();

        let status_label = Label::builder()
            .label("就绪 - [Esc] 或点击外部退出")
            .css_classes(["vok-status"])
            .halign(Align::End)
            .hexpand(true)
            .build();

        header_box.append(&title_label);
        header_box.append(&status_label);
        root_box.append(&header_box);

        // 中间拖拽投递区域 (Drop Target Area / Stage 货架原型)
        let drop_zone = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .css_classes(["vok-drop-zone"])
            .halign(Align::Center)
            .valign(Align::Center)
            .hexpand(true)
            .vexpand(true)
            .spacing(16)
            .build();

        let drop_icon_label = Label::builder()
            .label("📥")
            .css_classes(["vok-drop-icon"])
            .build();

        let drop_hint_label = Label::builder()
            .label("在外部文件管理器抓取文件 -> 在此释放即可入站中转")
            .css_classes(["vok-drop-hint"])
            .build();

        let dropped_files_label = Label::builder()
            .label("暂无文件入站")
            .css_classes(["vok-dropped-files"])
            .build();

        drop_zone.append(&drop_icon_label);
        drop_zone.append(&drop_hint_label);
        drop_zone.append(&dropped_files_label);
        root_box.append(&drop_zone);

        // 底部提示
        let footer_label = Label::builder()
            .label(format!("当前工作区几何: {} (已避让系统任务栏)", geom))
            .css_classes(["vok-footer"])
            .margin_bottom(16)
            .build();
        root_box.append(&footer_label);

        window.set_child(Some(&root_box));

        let on_dismiss: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_drop: Rc<RefCell<Option<Box<dyn Fn(Vec<PathBuf>)>>>> = Rc::new(RefCell::new(None));

        // 1. 注册拖拽丢入目标 GtkDropTarget
        let drop_target = DropTarget::new(FileList::static_type(), DragAction::COPY);
        let on_drop_cb = on_drop.clone();
        let dropped_label_clone = dropped_files_label.clone();
        drop_target.connect_drop(move |_target, value, _x, _y| {
            if let Ok(file_list) = value.get::<FileList>() {
                let mut paths = Vec::new();
                for file in file_list.files() {
                    if let Some(path) = file.path() {
                        paths.push(path);
                    }
                }
                info!("【外部拖入入站成功】捕获到 {} 个文件: {:?}", paths.len(), paths);
                dropped_label_clone.set_label(&format!(
                    "最新入站: {} (共 {} 项)",
                    paths.first().map(|p| p.display().to_string()).unwrap_or_default(),
                    paths.len()
                ));

                if let Some(cb) = on_drop_cb.borrow().as_ref() {
                    cb(paths);
                }
                true
            } else {
                warn!("收到未知格式拖放数据");
                false
            }
        });
        window.add_controller(drop_target);

        // 2. 注册 Esc 快捷键控制器
        let key_controller = EventControllerKey::new();
        let on_dismiss_esc = on_dismiss.clone();
        key_controller.connect_key_pressed(move |_ctrl, keyval, _keycode, _state| {
            if keyval == Key::Escape {
                debug!("用户在工作台按下 Esc 键");
                if let Some(cb) = on_dismiss_esc.borrow().as_ref() {
                    cb();
                }
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        window.add_controller(key_controller);

        // 3. 点击背景空白处关闭
        let click_gesture = GestureClick::new();
        let on_dismiss_click = on_dismiss.clone();
        click_gesture.connect_released(move |_gesture, _n_press, _x, _y| {
            debug!("用户点击工作台空白背景");
            if let Some(cb) = on_dismiss_click.borrow().as_ref() {
                cb();
            }
        });
        root_box.add_controller(click_gesture);

        Self {
            window,
            status_label,
            dropped_files_label,
            on_dismiss,
            on_drop,
        }
    }

    /// 设置关闭回调
    pub fn set_on_dismiss<F: Fn() + 'static>(&self, callback: F) {
        *self.on_dismiss.borrow_mut() = Some(Box::new(callback));
    }

    /// 设置拖入完成回调
    pub fn set_on_drop<F: Fn(Vec<PathBuf>) + 'static>(&self, callback: F) {
        *self.on_drop.borrow_mut() = Some(Box::new(callback));
    }

    /// 呈现窗口
    pub fn reveal(&self, gen: GenerationId) {
        self.status_label.set_label(&format!("会话中 [{gen}]"));
        self.window.present();
    }

    /// 隐藏窗口
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// 是否可见
    #[allow(dead_code)]
    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// 应用现代深色毛玻璃 CSS 样式
    fn apply_styles(window: &ApplicationWindow) {
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(
            "
            window {
                background-color: transparent;
            }
            .vok-root-container {
                background-color: rgba(18, 20, 28, 0.88);
                border: 1px solid rgba(255, 255, 255, 0.1);
            }
            .vok-title {
                color: #ffffff;
                font-size: 20px;
                font-weight: bold;
                letter-spacing: 1px;
            }
            .vok-status {
                color: #9aa0a6;
                font-size: 13px;
            }
            .vok-drop-zone {
                min-width: 500px;
                min-height: 280px;
                background-color: rgba(255, 255, 255, 0.04);
                border: 2px dashed rgba(255, 255, 255, 0.25);
                border-radius: 20px;
                padding: 32px;
            }
            .vok-drop-zone:hover {
                background-color: rgba(255, 255, 255, 0.08);
                border-color: #4da3ff;
            }
            .vok-drop-icon {
                font-size: 48px;
            }
            .vok-drop-hint {
                color: #e8eaed;
                font-size: 16px;
                font-weight: 500;
            }
            .vok-dropped-files {
                color: #8ab4f8;
                font-size: 14px;
            }
            .vok-footer {
                color: #5f6368;
                font-size: 12px;
            }
            ",
        );

        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        window.add_css_class("vok-window");
    }
}
