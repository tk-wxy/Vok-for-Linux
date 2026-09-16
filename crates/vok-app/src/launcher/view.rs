use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk4::gdk::{DragAction, FileList};
use gtk4::gio;
use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, DropTarget, GestureClick, GridView, Image, Label, Orientation,
    PolicyType, Popover, ScrolledWindow, SignalListItemFactory, SingleSelection,
};
use tracing::{debug, error, info, warn};
use vok_core::model::LauncherItem;
use vok_infra::desktop::DesktopEntry;
use vok_infra::storage::StorageClient;

use super::actions;
use super::model::LauncherObject;

type DismissCallback = Box<dyn Fn()>;

/// 启动台 4 列网格视图组件 (具备虚拟滚动与视口回收机制)
pub struct LauncherView {
    scrolled_window: ScrolledWindow,
    #[allow(dead_code)]
    grid_view: GridView,
    store: gio::ListStore,
    storage: StorageClient,
    #[allow(dead_code)]
    on_dismiss: Rc<RefCell<Option<DismissCallback>>>,
}

impl LauncherView {
    pub fn new(storage: StorageClient, on_dismiss: Rc<RefCell<Option<DismissCallback>>>) -> Self {
        let store = gio::ListStore::new::<LauncherObject>();
        let selection_model = SingleSelection::new(Some(store.clone()));
        selection_model.set_autoselect(false);
        selection_model.set_can_unselect(true);

        // 1. 构建视口回收卡片工厂 (SignalListItemFactory)
        let factory = SignalListItemFactory::new();

        let store_setup = store.clone();
        let storage_setup = storage.clone();
        let on_dismiss_setup = on_dismiss.clone();

        // 仅在视口池中按需创建极少数 Widget
        factory.connect_setup(move |_factory, list_item| {
            let item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();

            let card_box = GtkBox::builder()
                .orientation(Orientation::Vertical)
                .css_classes(["vok-launcher-card"])
                .spacing(6)
                .halign(Align::Center)
                .valign(Align::Center)
                .hexpand(true)
                .vexpand(true)
                .build();

            let icon = Image::builder()
                .pixel_size(36)
                .css_classes(["vok-launcher-card-icon"])
                .build();

            let label = Label::builder()
                .css_classes(["vok-launcher-card-title"])
                .ellipsize(EllipsizeMode::End)
                .max_width_chars(8)
                .lines(1)
                .build();

            card_box.append(&icon);
            card_box.append(&label);

            // 挂载右键上下文菜单手势
            let gesture = GestureClick::builder().button(3).build();
            let item_weak = item.downgrade();
            let store_clone = store_setup.clone();
            let storage_clone = storage_setup.clone();
            let on_dismiss_clone = on_dismiss_setup.clone();

            gesture.connect_pressed(move |_gesture, _n_press, _x, _y| {
                let Some(list_item) = item_weak.upgrade() else { return };
                let Some(obj) = list_item.item().and_downcast::<LauncherObject>() else { return };
                let launcher_item = obj.item();
                let Some(card) = list_item.child().and_downcast::<GtkBox>() else { return };
                Self::show_context_menu(
                    &card,
                    &launcher_item,
                    store_clone.clone(),
                    storage_clone.clone(),
                    on_dismiss_clone.clone(),
                );
            });
            card_box.add_controller(gesture);

            item.set_child(Some(&card_box));
        });

        // 绑定数据到视口中的 Widget
        factory.connect_bind(move |_factory, list_item| {
            let item = list_item.downcast_ref::<gtk4::ListItem>().unwrap();
            if let Some(obj) = item.item().and_downcast::<LauncherObject>() {
                let launcher_item = obj.item();
                if let Some(card_box) = item.child().and_downcast::<GtkBox>() {
                    if let Some(icon) = card_box.first_child().and_downcast::<Image>() {
                        let icon_name = &launcher_item.icon_name;
                        if icon_name.contains('/') && Path::new(icon_name).exists() {
                            icon.set_from_file(Some(icon_name));
                        } else if !icon_name.is_empty() {
                            icon.set_icon_name(Some(icon_name));
                        } else {
                            icon.set_icon_name(Some("application-x-executable"));
                        }
                    }

                    if let Some(label) = card_box.last_child().and_downcast::<Label>() {
                        label.set_label(&launcher_item.title);
                    }

                    card_box.set_tooltip_text(Some(&format!(
                        "{}\n目标: {}",
                        launcher_item.title, launcher_item.target
                    )));
                }
            }
        });

        // 2. 构建固定 4 列的 GtkGridView
        let grid_view = GridView::builder()
            .model(&selection_model)
            .factory(&factory)
            .min_columns(4)
            .max_columns(4)
            .enable_rubberband(false)
            .single_click_activate(true)
            .css_classes(["vok-launcher-grid"])
            .build();

        // 3. 左键单击启动并隐藏全屏浮层
        {
            let store_clone = store.clone();
            let on_dismiss_clone = on_dismiss.clone();
            grid_view.connect_activate(move |_view, position| {
                if let Some(obj) = store_clone.item(position).and_downcast::<LauncherObject>() {
                    let item = obj.item();
                    info!("【启动台激活】启动应用: {} ({})", item.title, item.target);
                    if let Err(e) = actions::launch_target(&item.target) {
                        error!("启动应用失败: {}", e);
                    }

                    // 立即交回焦点并隐藏工作台
                    if let Some(cb) = on_dismiss_clone.borrow().as_ref() {
                        cb();
                    }
                }
            });
        }

        // 4. 滚动视口包装
        let scrolled_window = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .child(&grid_view)
            .hexpand(true)
            .vexpand(true)
            .build();

        // 6. 挂载外部拖拽添加 GtkDropTarget
        Self::attach_drop_target(&scrolled_window, store.clone(), storage.clone());

        let view = Self {
            scrolled_window,
            grid_view,
            store,
            storage,
            on_dismiss,
        };

        // 自动载入数据
        view.load_items();

        view
    }

    pub fn widget(&self) -> &ScrolledWindow {
        &self.scrolled_window
    }

    /// 从数据库重载所有启动项
    pub fn load_items(&self) {
        // 若数据库为空则初始化默认应用
        actions::seed_default_launcher_items(&self.storage);

        self.store.remove_all();
        match self.storage.list_launcher_items() {
            Ok(items) => {
                debug!("从数据库载入 {} 个启动项", items.len());
                for item in items {
                    self.store.append(&LauncherObject::new(item));
                }
            }
            Err(e) => {
                error!("载入启动项失败: {}", e);
            }
        }
    }

    /// 弹出右键上下文菜单
    fn show_context_menu(
        parent: &GtkBox,
        item: &LauncherItem,
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

        // 菜单项 1: 启动
        let btn_launch = Button::builder().label("🚀 立即启动").has_frame(false).build();
        {
            let target = item.target.clone();
            let pop_clone = popover.clone();
            let on_dismiss_clone = on_dismiss.clone();
            btn_launch.connect_clicked(move |_| {
                pop_clone.popdown();
                let _ = actions::launch_target(&target);
                if let Some(cb) = on_dismiss_clone.borrow().as_ref() {
                    cb();
                }
            });
        }

        // 菜单项 2: 打开所在目录
        let btn_folder = Button::builder().label("📂 打开所在目录").has_frame(false).build();
        {
            let target = item.target.clone();
            let pop_clone = popover.clone();
            btn_folder.connect_clicked(move |_| {
                pop_clone.popdown();
                let _ = actions::show_in_folder(&target);
            });
        }

        // 菜单项 3: 从启动台移除
        let btn_remove = Button::builder().label("🗑 移除启动项").has_frame(false).build();
        {
            let item_id = item.id;
            let store_clone = store.clone();
            let storage_clone = storage.clone();
            let pop_clone = popover.clone();
            btn_remove.connect_clicked(move |_| {
                pop_clone.popdown();
                if let Err(e) = storage_clone.delete_launcher_item(item_id) {
                    error!("数据库删除启动项失败: {}", e);
                } else {
                    info!("已从启动台移除项目 id={}", item_id);
                    // 从界面模型中移除
                    for idx in 0..store_clone.n_items() {
                        if let Some(o) = store_clone.item(idx).and_downcast::<LauncherObject>() {
                            if o.id() == item_id {
                                store_clone.remove(idx);
                                break;
                            }
                        }
                    }
                }
            });
        }

        menu_box.append(&btn_launch);
        menu_box.append(&btn_folder);
        menu_box.append(&btn_remove);

        popover.set_child(Some(&menu_box));
        popover.popup();
    }

    /// 挂载外部拖拽添加控制器
    fn attach_drop_target(
        widget: &impl IsA<gtk4::Widget>,
        store: gio::ListStore,
        storage: StorageClient,
    ) {
        let drop_target = DropTarget::new(FileList::static_type(), DragAction::COPY);

        drop_target.connect_drop(move |_target, value, _x, _y| {
            if let Ok(file_list) = value.get::<FileList>() {
                for file in file_list.files() {
                    if let Some(path) = file.path() {
                        Self::add_external_path(&path, &store, &storage);
                    }
                }
                true
            } else {
                false
            }
        });

        widget.add_controller(drop_target);
    }

    fn add_external_path(path: &Path, store: &gio::ListStore, storage: &StorageClient) {
        info!("【启动台外部拖入】正在解析添加: {:?}", path);

        let item = if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
            if let Ok(entry) = DesktopEntry::from_file(path) {
                LauncherItem::new(
                    0,
                    entry.name,
                    path.to_string_lossy().to_string(),
                    entry.icon,
                    store.n_items() as i32,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64,
                )
            } else {
                warn!("无法解析 .desktop 文件: {:?}", path);
                return;
            }
        } else {
            // 普通文件或脚本
            let filename = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "未命名".to_string());
            LauncherItem::new(
                0,
                filename,
                path.to_string_lossy().to_string(),
                "application-x-executable",
                store.n_items() as i32,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            )
        };

        match storage.add_launcher_item(item.clone()) {
            Ok(new_id) => {
                let mut saved_item = item;
                saved_item.id = new_id;
                store.append(&LauncherObject::new(saved_item));
                info!("成功添加新启动项到启动台: {}", new_id);
            }
            Err(e) => {
                error!("持久化新启动项失败: {}", e);
            }
        }
    }
}
