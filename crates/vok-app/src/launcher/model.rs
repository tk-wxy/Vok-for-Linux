use std::cell::RefCell;

use gtk4::glib;
use gtk4::glib::subclass::prelude::*;
use vok_core::model::LauncherItem;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct LauncherObject {
        pub item: RefCell<Option<LauncherItem>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LauncherObject {
        const NAME: &'static str = "VokLauncherObject";
        type Type = super::LauncherObject;
    }

    impl ObjectImpl for LauncherObject {}
}

glib::wrapper! {
    /// 启动台 GObject 包装器，用于接入 GListStore 与 GtkGridView 虚拟视口回收管线
    pub struct LauncherObject(ObjectSubclass<imp::LauncherObject>);
}

impl LauncherObject {
    pub fn new(item: LauncherItem) -> Self {
        let obj: Self = glib::Object::new();
        *obj.imp().item.borrow_mut() = Some(item);
        obj
    }

    pub fn item(&self) -> LauncherItem {
        self.imp()
            .item
            .borrow()
            .clone()
            .expect("LauncherItem 必须已初始化")
    }

    pub fn id(&self) -> i64 {
        self.item().id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launcher_object_wrapper() {
        if !gtk4::is_initialized() {
            let _ = gtk4::init();
        }

        let item = LauncherItem::new(
            42,
            "Firefox".to_string(),
            "/usr/bin/firefox".to_string(),
            "firefox".to_string(),
            0,
            12345678,
        );

        let obj = LauncherObject::new(item.clone());
        assert_eq!(obj.id(), 42);
        assert_eq!(obj.item().title, "Firefox");
        assert_eq!(obj.item().target, "/usr/bin/firefox");
        assert_eq!(obj.item().icon_name, "firefox");
    }
}
