use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, Orientation, ScrolledWindow,
};
use vok_core::layout::{ComputedCardLayout, ComputedThreePaneLayout};
use vok_infra::desktop::ThumbnailWorker;
use vok_infra::storage::StorageClient;

use super::adapter::LayoutAdapter;
use crate::launcher::LauncherView;
use crate::stage::StageView;

type DropCallback = Box<dyn Fn(Vec<PathBuf>)>;
type DismissCallback = Box<dyn Fn()>;

/// 1:2:1 主界面水平三栏容器组件
pub struct ThreePaneContainer {
    container: GtkBox,
    launcher_pane: GtkBox,
    stage_pane: GtkBox,
    clipboard_pane: GtkBox,
    #[allow(dead_code)]
    launcher_view: LauncherView,
    #[allow(dead_code)]
    stage_view: StageView,
    #[allow(dead_code)]
    on_drop: Rc<RefCell<Option<DropCallback>>>,
}

impl ThreePaneContainer {
    pub fn new(
        storage: StorageClient,
        thumbnail_worker: Arc<ThumbnailWorker>,
        on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
    ) -> Self {
        let container = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["vok-three-pane"])
            .spacing(16)
            .hexpand(true)
            .vexpand(true)
            .build();

        let on_drop: Rc<RefCell<Option<DropCallback>>> = Rc::new(RefCell::new(None));

        // 1. 左栏：启动台 (25%) - 接入真实 LauncherView
        let (launcher_pane, launcher_view) = Self::build_launcher_pane(storage.clone(), on_dismiss.clone());

        // 2. 中栏：临时中转站 (50%) - 接入真实 StageView
        let (stage_pane, stage_view) = Self::build_stage_pane(storage, thumbnail_worker, on_dismiss);

        // 3. 右栏：剪贴板占位区 (25%)
        let (clipboard_pane, _clip_content) = Self::build_clipboard_pane();

        container.append(&launcher_pane);
        container.append(&stage_pane);
        container.append(&clipboard_pane);

        Self {
            container,
            launcher_pane,
            stage_pane,
            clipboard_pane,
            launcher_view,
            stage_view,
            on_drop,
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    #[allow(dead_code)]
    pub fn launcher_view(&self) -> &LauncherView {
        &self.launcher_view
    }

    #[allow(dead_code)]
    pub fn stage_view(&self) -> &StageView {
        &self.stage_view
    }

    pub fn set_on_drop<F: Fn(Vec<PathBuf>) + 'static>(&self, callback: F) {
        *self.on_drop.borrow_mut() = Some(Box::new(callback));
    }

    /// 应用动态几何宽度分配与垂直高度整除
    pub fn apply_geometry(
        &self,
        width: u32,
        height: u32,
    ) -> (ComputedThreePaneLayout, ComputedCardLayout) {
        LayoutAdapter::apply_layout(
            width,
            height,
            &self.launcher_pane,
            &self.stage_pane,
            &self.clipboard_pane,
        )
    }

    // ==================== 内部构建辅助函数 ====================

    fn build_launcher_pane(
        storage: StorageClient,
        on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
    ) -> (GtkBox, LauncherView) {
        let pane = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .css_classes(["vok-pane", "vok-pane-launcher"])
            .vexpand(true)
            .build();

        // 栏头
        let header = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["vok-pane-header"])
            .spacing(8)
            .build();

        let title = Label::builder()
            .label("🚀 启动台")
            .css_classes(["vok-pane-title"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let badge = Label::builder()
            .label("4 列网格")
            .css_classes(["vok-pane-badge"])
            .halign(Align::End)
            .build();

        header.append(&title);
        header.append(&badge);
        pane.append(&header);

        // 实例化原生 4 列网格虚拟滚动 LauncherView
        let launcher_view = LauncherView::new(storage, on_dismiss);
        pane.append(launcher_view.widget());

        (pane, launcher_view)
    }

    fn build_stage_pane(
        storage: StorageClient,
        thumbnail_worker: Arc<ThumbnailWorker>,
        on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
    ) -> (GtkBox, StageView) {
        let pane = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .css_classes(["vok-pane", "vok-pane-stage"])
            .hexpand(true)
            .vexpand(true)
            .build();

        // 实例化原生 2 列网格虚拟滚动 StageView
        let stage_view = StageView::new(storage, thumbnail_worker, on_dismiss);

        // 栏头
        let header = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["vok-pane-header"])
            .spacing(8)
            .build();

        let title = Label::builder()
            .label("📥 临时中转站")
            .css_classes(["vok-pane-title"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let btn_clear = Button::builder()
            .label("🧹 清理未置顶")
            .css_classes(["vok-btn-clear"])
            .halign(Align::End)
            .build();

        {
            let view_clone = stage_view.clone();
            btn_clear.connect_clicked(move |_| {
                view_clone.clear_unpinned();
            });
        }

        header.append(&title);
        header.append(&btn_clear);
        pane.append(&header);

        // 嵌入中转站虚拟滚动视图
        pane.append(stage_view.widget());

        (pane, stage_view)
    }

    fn build_clipboard_pane() -> (GtkBox, GtkBox) {
        let pane = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .css_classes(["vok-pane", "vok-pane-clipboard"])
            .vexpand(true)
            .build();

        // 栏头
        let header = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["vok-pane-header"])
            .spacing(8)
            .build();

        let title = Label::builder()
            .label("📋 剪贴板历史")
            .css_classes(["vok-pane-title"])
            .halign(Align::Start)
            .hexpand(true)
            .build();

        let badge = Label::builder()
            .label("M3 预留")
            .css_classes(["vok-pane-badge"])
            .halign(Align::End)
            .build();

        header.append(&title);
        header.append(&badge);
        pane.append(&header);

        // 骨架容器
        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();

        let content = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .build();

        let placeholder = Label::builder()
            .label("剪贴板历史与单键直接粘贴\n[M3 里程碑全面就绪]")
            .css_classes(["vok-card-skeleton"])
            .margin_top(24)
            .build();
        content.append(&placeholder);

        scroll.set_child(Some(&content));
        pane.append(&scroll);

        (pane, content)
    }
}
