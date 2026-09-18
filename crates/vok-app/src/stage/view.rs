use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gtk4::gdk::{ContentProvider, DragAction, FileList};
use gtk4::gio;
use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, DragSource, DropTarget, GestureClick, GridView, Image, Label,
    Orientation, PolicyType, Popover, ScrolledWindow, SignalListItemFactory, SingleSelection,
};
use tracing::{debug, error, info};
use vok_core::model::{StageItem, StageItemKind};
use vok_infra::desktop::ThumbnailWorker;
use vok_infra::storage::StorageClient;

use super::actions;
use super::model::StageObject;

type DismissCallback = Box<dyn Fn()>;
type DragCallback = Box<dyn Fn()>;

/// 中转站 2 列网格视图组件 (具备虚拟滚动、双向拖拽与异步缩略图渲染)
#[derive(Clone)]
pub struct StageView {
    scrolled_window: ScrolledWindow,
    #[allow(dead_code)]
    grid_view: GridView,
    store: gio::ListStore,
    storage: StorageClient,
    #[allow(dead_code)]
    thumbnail_worker: Arc<ThumbnailWorker>,
    #[allow(dead_code)]
    on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
    on_drag_begin: Rc<RefCell<Option<DragCallback>>>,
    on_drag_end: Rc<RefCell<Option<DragCallback>>>,
}

impl StageView {
    pub fn new(
        storage: StorageClient,
        thumbnail_worker: Arc<ThumbnailWorker>,
        on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
    ) -> Self {
        let store = gio::ListStore::new::<StageObject>();
        let selection_model = SingleSelection::new(Some(store.clone()));
        selection_model.set_autoselect(false);
        selection_model.set_can_unselect(true);

        // 1. 构建视口回收卡片工厂 (SignalListItemFactory)
        let factory = SignalListItemFactory::new();

        let store_setup = store.clone();
        let storage_setup = storage.clone();
        let on_dismiss_setup = on_dismiss.clone();
        let on_drag_begin: Rc<RefCell<Option<DragCallback>>> = Rc::new(RefCell::new(None));
        let on_drag_end: Rc<RefCell<Option<DragCallback>>> = Rc::new(RefCell::new(None));
        let on_drag_begin_setup = on_drag_begin.clone();
        let on_drag_end_setup = on_drag_end.clone();

        // 创建异步缩略图投递通道 (后台线程发送，GTK 主线程安全消费)
        let (thumb_tx, thumb_rx) = async_channel::unbounded::<(i64, PathBuf)>();

        // 在 UI 线程异步监听缩略图生成完成并安全刷新 ListStore
        {
            let store_thumbs = store.clone();
            let storage_thumbs = storage.clone();
            gtk4::glib::spawn_future_local(async move {
                while let Ok((id, thumb_path)) = thumb_rx.recv().await {
                    let _ = storage_thumbs.update_stage_thumbnail(id, thumb_path.clone());
                    for idx in 0..store_thumbs.n_items() {
                        if let Some(obj) = store_thumbs.item(idx).and_downcast::<StageObject>() {
                            if obj.id() == id {
                                obj.set_thumbnail(Some(thumb_path));
                                store_thumbs.items_changed(idx, 1, 1);
                                break;
                            }
                        }
                    }
                }
            });
        }

        factory.connect_setup(move |_factory, list_item| {
            let item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();

            // 主卡片水平容器
            let card_box = GtkBox::builder()
                .orientation(Orientation::Horizontal)
                .css_classes(["vok-stage-card"])
                .spacing(14)
                .hexpand(true)
                .vexpand(true)
                .build();

            // 左侧：缩略图/图标预览区
            let preview_box = GtkBox::builder()
                .css_classes(["vok-stage-preview-box"])
                .halign(Align::Center)
                .valign(Align::Center)
                .build();

            let icon = Image::builder()
                .pixel_size(56)
                .css_classes(["vok-stage-card-icon"])
                .build();
            preview_box.append(&icon);

            // 右侧：多行元数据纵向区
            let meta_box = GtkBox::builder()
                .orientation(Orientation::Vertical)
                .spacing(4)
                .hexpand(true)
                .valign(Align::Center)
                .build();

            // 标题行 (文件名 + 置顶徽章)
            let title_row = GtkBox::builder()
                .orientation(Orientation::Horizontal)
                .spacing(6)
                .build();

            let title_label = Label::builder()
                .css_classes(["vok-stage-card-title"])
                .ellipsize(EllipsizeMode::End)
                .max_width_chars(16)
                .hexpand(true)
                .halign(Align::Start)
                .build();

            let pin_badge = Label::builder()
                .label("📌")
                .css_classes(["vok-stage-pin-badge"])
                .visible(false)
                .build();

            title_row.append(&title_label);
            title_row.append(&pin_badge);

            // 属性行 (大小 + 类别)
            let meta_label = Label::builder()
                .css_classes(["vok-stage-card-meta"])
                .halign(Align::Start)
                .build();

            // 物理路径行 (暗色居底)
            let path_label = Label::builder()
                .css_classes(["vok-stage-card-path"])
                .ellipsize(EllipsizeMode::Middle)
                .max_width_chars(20)
                .hexpand(true)
                .halign(Align::Start)
                .build();

            meta_box.append(&title_row);
            meta_box.append(&meta_label);
            meta_box.append(&path_label);

            card_box.append(&preview_box);
            card_box.append(&meta_box);

            // 挂载反向拖出控制器 (GtkDragSource - 遵循 R3 透明代理锚点铁律)
            let drag_source = DragSource::builder()
                .actions(DragAction::COPY)
                .build();
            let item_weak_for_drag = item.downgrade();
            drag_source.connect_prepare(move |_source, _x, _y| {
                let item = item_weak_for_drag.upgrade()?;
                let obj = item.item().and_downcast::<StageObject>()?;
                let source_path = obj.item().path;
                let file = gio::File::for_path(&source_path);
                let file_list = FileList::from_array(&[file]);
                let file_provider = ContentProvider::for_value(&file_list.to_value());
                let text_provider = ContentProvider::for_value(&source_path.to_string_lossy().to_string().to_value());
                Some(ContentProvider::new_union(&[file_provider, text_provider]))
            });

            let on_drag_begin_card = on_drag_begin_setup.clone();
            let on_drag_end_card1 = on_drag_end_setup.clone();
            let on_drag_end_card2 = on_drag_end_setup.clone();

            drag_source.connect_drag_begin(move |_source, _drag| {
                info!("【反向拖出起手】GtkDragSource 触发 drag-begin，激活透明代理锚点 (规则 R3)");
                if let Some(cb) = on_drag_begin_card.borrow().as_ref() {
                    cb();
                }
            });

            drag_source.connect_drag_end(move |_source, _drag, _delete_data| {
                info!("【反向拖出结束】GtkDragSource 触发 drag-end，退出透明代理并隐退");
                if let Some(cb) = on_drag_end_card1.borrow().as_ref() {
                    cb();
                }
            });

            drag_source.connect_drag_cancel(move |_source, _drag, reason| {
                info!("【反向拖出取消】GtkDragSource 触发 drag-cancel (原因: {:?})，退出透明代理并隐退", reason);
                if let Some(cb) = on_drag_end_card2.borrow().as_ref() {
                    cb();
                }
                false
            });

            card_box.add_controller(drag_source);

            // 挂载右键上下文菜单手势
            let gesture = GestureClick::builder().button(3).build();
            let item_weak_for_menu = item.downgrade();
            let store_clone = store_setup.clone();
            let storage_clone = storage_setup.clone();
            let on_dismiss_clone = on_dismiss_setup.clone();

            gesture.connect_pressed(move |_gesture, _n_press, _x, _y| {
                let Some(list_item) = item_weak_for_menu.upgrade() else { return };
                let Some(obj) = list_item.item().and_downcast::<StageObject>() else { return };
                let stage_item = obj.item();
                let Some(card) = list_item.child().and_downcast::<GtkBox>() else { return };
                Self::show_context_menu(
                    &card,
                    &stage_item,
                    &obj,
                    store_clone.clone(),
                    storage_clone.clone(),
                    on_dismiss_clone.clone(),
                );
            });
            card_box.add_controller(gesture);

            item.set_child(Some(&card_box));
        });

        // 绑定数据到视口卡片
        let worker_bind = thumbnail_worker.clone();
        let thumb_tx_bind = thumb_tx.clone();

        factory.connect_bind(move |_factory, list_item| {
            let item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            if let Some(obj) = item.item().and_downcast::<StageObject>() {
                let stage_item = obj.item();
                if let Some(card_box) = item.child().and_downcast::<GtkBox>() {
                    // 置顶边框样式联动
                    if stage_item.pinned {
                        card_box.add_css_class("vok-stage-card-pinned");
                    } else {
                        card_box.remove_css_class("vok-stage-card-pinned");
                    }

                    // 坏链检测 (R4 纯引用：外部文件可能已被移动或删除)
                    let file_exists = stage_item.path.exists();
                    let path_str = stage_item.path.to_string_lossy().to_string();
                    if !file_exists {
                        card_box.add_css_class("vok-stage-card-broken");
                        card_box.set_tooltip_text(Some(&format!(
                            "{}\n⚠️ 原文件已不存在或已被外部移动\n物理路径: {}",
                            stage_item.display_name, path_str
                        )));
                    } else {
                        card_box.remove_css_class("vok-stage-card-broken");
                        card_box.set_tooltip_text(Some(&format!(
                            "{}\n物理路径: {}",
                            stage_item.display_name, path_str
                        )));
                    }

                    // 提取子元素
                    let preview_box = card_box.first_child().and_downcast::<GtkBox>().unwrap();
                    let icon = preview_box.first_child().and_downcast::<Image>().unwrap();

                    let meta_box = card_box.last_child().and_downcast::<GtkBox>().unwrap();
                    let title_row = meta_box.first_child().and_downcast::<GtkBox>().unwrap();
                    let title_label = title_row.first_child().and_downcast::<Label>().unwrap();
                    let pin_badge = title_row.last_child().and_downcast::<Label>().unwrap();
                    let meta_label = meta_box.observe_children().item(1).unwrap().downcast::<Label>().unwrap();
                    let path_label = meta_box.last_child().and_downcast::<Label>().unwrap();

                    // 填充标题与路径
                    title_label.set_label(&stage_item.display_name);
                    pin_badge.set_visible(stage_item.pinned);
                    path_label.set_label(&path_str);

                    // 格式化大小与类别
                    let size_str = actions::format_file_size(stage_item.size_bytes);
                    let kind_str = match stage_item.kind {
                        StageItemKind::Image => "图片",
                        StageItemKind::Text => "文本",
                        StageItemKind::File => "文件",
                    };
                    meta_label.set_label(&format!("{} · {}", size_str, kind_str));

                    // 图像缩略图异步生成与渲染管线
                    Self::bind_icon_or_thumbnail(
                        &stage_item,
                        &icon,
                        &worker_bind,
                        &thumb_tx_bind,
                    );
                }
            }
        });

        // 2. 构建 2 列网格 GtkGridView
        let grid_view = GridView::builder()
            .model(&selection_model)
            .factory(&factory)
            .min_columns(2)
            .max_columns(2)
            .enable_rubberband(false)
            .single_click_activate(true)
            .css_classes(["vok-stage-grid"])
            .build();

        // 3. 左键单击/回车激活直接粘贴至原前台光标处
        {
            let store_clone = store.clone();
            let on_dismiss_clone = on_dismiss.clone();
            grid_view.connect_activate(move |_view, position| {
                if let Some(obj) = store_clone.item(position).and_downcast::<StageObject>() {
                    let item = obj.item();
                    info!("【中转站激活】直接粘贴文件至原前台: {:?}", item.path);
                    let dismiss_ref = on_dismiss_clone.borrow();
                    let dismiss_fn = dismiss_ref.as_ref().map(|b| &**b as &dyn Fn());
                    actions::direct_paste_item_to_foreground(&item.path, dismiss_fn);
                }
            });
        }

        // 4. 滚动视口包装
        let scrolled_window = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .propagate_natural_width(false)
            .propagate_natural_height(false)
            .child(&grid_view)
            .hexpand(true)
            .vexpand(true)
            .build();

        // 5. 挂载外部拖入 GtkDropTarget
        Self::attach_drop_target(&scrolled_window, store.clone(), storage.clone());

        let view = Self {
            scrolled_window,
            grid_view,
            store,
            storage,
            thumbnail_worker,
            on_dismiss,
            on_drag_begin,
            on_drag_end,
        };

        // 载入持久化数据
        view.load_items();

        view
    }

    pub fn widget(&self) -> &ScrolledWindow {
        &self.scrolled_window
    }

    #[allow(dead_code)]
    pub fn store(&self) -> &gio::ListStore {
        &self.store
    }

    /// 设置反向拖出起手回调 (进入透明代理锚点)
    pub fn set_on_drag_begin<F: Fn() + 'static>(&self, callback: F) {
        *self.on_drag_begin.borrow_mut() = Some(Box::new(callback));
    }

    /// 设置反向拖出结束/取消回调 (退出透明代理并安全隐退)
    pub fn set_on_drag_end<F: Fn() + 'static>(&self, callback: F) {
        *self.on_drag_end.borrow_mut() = Some(Box::new(callback));
    }

    /// 从持久化数据库载入全部中转站文件 (置顶优先排布)
    pub fn load_items(&self) {
        self.store.remove_all();
        match self.storage.list_stage_items() {
            Ok(mut items) => {
                debug!("从数据库载入 {} 个中转站项目", items.len());
                // 置顶项优先，其后按 order_index 升序
                items.sort_by(|a, b| {
                    b.pinned
                        .cmp(&a.pinned)
                        .then_with(|| a.order_index.cmp(&b.order_index))
                });
                for item in items {
                    self.store.append(&StageObject::new(item));
                }
            }
            Err(e) => {
                error!("载入中转站项目失败: {}", e);
            }
        }
    }

    /// 清空所有未置顶项
    pub fn clear_unpinned(&self) {
        match self.storage.clear_unpinned_stage_items() {
            Ok(count) => {
                info!("已从数据库批量清空 {} 项未置顶中转文件", count);
                // 仅保留置顶项
                let mut idx = 0;
                while idx < self.store.n_items() {
                    if let Some(obj) = self.store.item(idx).and_downcast::<StageObject>() {
                        if !obj.is_pinned() {
                            self.store.remove(idx);
                            continue;
                        }
                    }
                    idx += 1;
                }
            }
            Err(e) => {
                error!("清空未置顶项失败: {}", e);
            }
        }
    }

    /// 缩略图与图标绑定逻辑
    fn bind_icon_or_thumbnail(
        stage_item: &StageItem,
        icon: &Image,
        worker: &Arc<ThumbnailWorker>,
        thumb_tx: &async_channel::Sender<(i64, PathBuf)>,
    ) {
        // 1. 若已有缩略图且文件存在，直接加载
        if let Some(ref thumb) = stage_item.thumbnail {
            if thumb.exists() {
                icon.set_from_file(Some(thumb));
                return;
            }
        }

        // 2. 如果是图片类型，异步投递生成
        if stage_item.kind == StageItemKind::Image {
            let path = stage_item.path.clone();
            if path.exists() {
                let mtime = stage_item.mtime_sec;
                let item_id = stage_item.id;
                let tx_clone = thumb_tx.clone();

                icon.set_icon_name(Some("image-x-generic"));

                worker.request_thumbnail(item_id, path, mtime, move |id, maybe_path| {
                    if let Some(thumb_path) = maybe_path {
                        let _ = tx_clone.send_blocking((id, thumb_path));
                    }
                });
                return;
            }
        }

        // 3. 其它文件格式，绑定系统主题矢量大图标
        let icon_name = match stage_item.kind {
            StageItemKind::Text => "text-x-generic",
            StageItemKind::Image => "image-x-generic",
            StageItemKind::File => {
                if stage_item.path.is_dir() {
                    "folder"
                } else {
                    "text-x-generic"
                }
            }
        };
        icon.set_icon_name(Some(icon_name));
    }

    /// 弹出右键上下文菜单
    fn show_context_menu(
        parent: &GtkBox,
        item: &StageItem,
        obj: &StageObject,
        store: gio::ListStore,
        storage: StorageClient,
        on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
    ) {
        let popover = Popover::builder().has_arrow(true).build();
        popover.set_parent(parent);
        popover.connect_closed(|p| {
            p.unparent();
        });

        let menu_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();

        // 菜单项 1: 直接粘贴至原前台
        let btn_paste = Button::builder().label("⚡ 直接粘贴 (Enter)").has_frame(false).build();
        {
            let path = item.path.clone();
            let pop_clone = popover.clone();
            let on_dismiss_clone = on_dismiss.clone();
            btn_paste.connect_clicked(move |_| {
                pop_clone.popdown();
                let dismiss_ref = on_dismiss_clone.borrow();
                let dismiss_fn = dismiss_ref.as_ref().map(|b| &**b as &dyn Fn());
                actions::direct_paste_item_to_foreground(&path, dismiss_fn);
            });
        }

        // 菜单项 2: 置顶 / 取消置顶
        let pin_label = if item.pinned {
            "📌 取消置顶"
        } else {
            "📌 设为置顶"
        };
        let btn_pin = Button::builder().label(pin_label).has_frame(false).build();
        {
            let item_id = item.id;
            let new_pinned = !item.pinned;
            let storage_clone = storage.clone();
            let pop_clone = popover.clone();
            let obj_clone = obj.clone();
            let store_clone = store.clone();
            btn_pin.connect_clicked(move |_| {
                pop_clone.popdown();
                if let Err(e) = storage_clone.set_stage_pinned(item_id, new_pinned) {
                    error!("更新置顶状态失败: {}", e);
                } else {
                    obj_clone.set_pinned(new_pinned);
                    info!("更新中转站项目 {} 置顶状态 -> {}", item_id, new_pinned);
                    Self::resort_store(&store_clone);
                }
            });
        }

        // 菜单项 2: 打开文件
        let btn_open = Button::builder().label("🚀 打开文件").has_frame(false).build();
        {
            let path = item.path.clone();
            let pop_clone = popover.clone();
            let on_dismiss_clone = on_dismiss.clone();
            btn_open.connect_clicked(move |_| {
                pop_clone.popdown();
                let _ = actions::open_item(&path);
                if let Some(cb) = on_dismiss_clone.borrow().as_ref() {
                    cb();
                }
            });
        }

        // 菜单项 3: 打开所在目录
        let btn_folder = Button::builder().label("📂 打开所在目录").has_frame(false).build();
        {
            let path = item.path.clone();
            let pop_clone = popover.clone();
            btn_folder.connect_clicked(move |_| {
                pop_clone.popdown();
                let _ = actions::show_in_folder(&path);
            });
        }

        // 菜单项 4: 复制文件物理路径
        let btn_copy = Button::builder().label("📋 复制物理路径").has_frame(false).build();
        {
            let path = item.path.clone();
            let pop_clone = popover.clone();
            btn_copy.connect_clicked(move |_| {
                pop_clone.popdown();
                actions::copy_path_to_clipboard(&path);
            });
        }

        // 菜单项 5: 从中转站移除
        let btn_remove = Button::builder().label("🗑 移出中转站").has_frame(false).build();
        {
            let item_id = item.id;
            let store_clone = store.clone();
            let storage_clone = storage.clone();
            let pop_clone = popover.clone();
            btn_remove.connect_clicked(move |_| {
                pop_clone.popdown();
                if let Err(e) = storage_clone.delete_stage_item(item_id) {
                    error!("数据库删除中转站项目失败: {}", e);
                } else {
                    info!("已从中转站移除项目 id={}", item_id);
                    for idx in 0..store_clone.n_items() {
                        if let Some(o) = store_clone.item(idx).and_downcast::<StageObject>() {
                            if o.id() == item_id {
                                store_clone.remove(idx);
                                break;
                            }
                        }
                    }
                }
            });
        }

        menu_box.append(&btn_paste);
        menu_box.append(&btn_pin);
        menu_box.append(&btn_open);
        menu_box.append(&btn_folder);
        menu_box.append(&btn_copy);
        menu_box.append(&btn_remove);

        popover.set_child(Some(&menu_box));
        popover.popup();
    }

    /// 挂载外部文件拖入控制器
    fn attach_drop_target(
        widget: &impl IsA<gtk4::Widget>,
        store: gio::ListStore,
        storage: StorageClient,
    ) {
        let drop_target = DropTarget::new(FileList::static_type(), DragAction::COPY);

        drop_target.connect_drop(move |_target, value, _x, _y| {
            if let Ok(file_list) = value.get::<FileList>() {
                let mut paths = Vec::new();
                for file in file_list.files() {
                    if let Some(path) = file.path() {
                        paths.push(path);
                    }
                }

                if !paths.is_empty() {
                    let base_order = store.n_items() as i32;
                    match actions::ingest_dropped_paths(&storage, &paths, base_order) {
                        Ok(new_items) => {
                            for item in new_items {
                                store.append(&StageObject::new(item));
                            }
                            Self::resort_store(&store);
                        }
                        Err(e) => {
                            error!("批量入站存储失败: {}", e);
                        }
                    }
                }
                true
            } else {
                false
            }
        });

        widget.add_controller(drop_target);
    }

    /// 根据置顶与原始次序重新整理 ListStore
    fn resort_store(store: &gio::ListStore) {
        let mut items = Vec::new();
        for idx in 0..store.n_items() {
            if let Some(obj) = store.item(idx).and_downcast::<StageObject>() {
                items.push(obj.item());
            }
        }

        items.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| a.order_index.cmp(&b.order_index))
        });

        store.remove_all();
        for item in items {
            store.append(&StageObject::new(item));
        }
    }
}
