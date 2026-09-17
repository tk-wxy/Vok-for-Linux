use std::cell::RefCell;
use std::path::PathBuf;

use gtk4::glib;
use gtk4::glib::subclass::prelude::*;
use vok_core::model::StageItem;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct StageObject {
        pub item: RefCell<Option<StageItem>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StageObject {
        const NAME: &'static str = "VokStageObject";
        type Type = super::StageObject;
    }

    impl ObjectImpl for StageObject {}
}

glib::wrapper! {
    /// 中转站 GObject 包装器，用于接入 GListStore 与 GtkGridView 虚拟视口回收
    pub struct StageObject(ObjectSubclass<imp::StageObject>);
}

impl StageObject {
    pub fn new(item: StageItem) -> Self {
        let obj: Self = glib::Object::new();
        *obj.imp().item.borrow_mut() = Some(item);
        obj
    }

    pub fn item(&self) -> StageItem {
        self.imp()
            .item
            .borrow()
            .clone()
            .expect("StageItem 必须已初始化")
    }

    pub fn set_item(&self, item: StageItem) {
        *self.imp().item.borrow_mut() = Some(item);
    }

    pub fn id(&self) -> i64 {
        self.item().id
    }

    pub fn is_pinned(&self) -> bool {
        self.item().pinned
    }

    pub fn set_pinned(&self, pinned: bool) {
        if let Some(ref mut item) = *self.imp().item.borrow_mut() {
            item.pinned = pinned;
        }
    }

    pub fn thumbnail(&self) -> Option<PathBuf> {
        self.item().thumbnail
    }

    pub fn set_thumbnail(&self, thumb: Option<PathBuf>) {
        if let Some(ref mut item) = *self.imp().item.borrow_mut() {
            item.thumbnail = thumb;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_stage_object_wrapper_and_mutations() {
        if !gtk4::is_initialized() {
            let _ = gtk4::init();
        }

        let item = StageItem::from_path_ref(
            100,
            Path::new("/home/user/sample.png"),
            "sample.png",
            1024 * 1024,
            "image/png",
            1600000000,
            0,
            1234567,
        );

        let obj = StageObject::new(item);
        assert_eq!(obj.id(), 100);
        assert_eq!(obj.item().display_name, "sample.png");
        assert!(!obj.is_pinned());
        assert_eq!(obj.thumbnail(), None);

        // 测试置顶变更
        obj.set_pinned(true);
        assert!(obj.is_pinned());

        // 测试缩略图路径变更
        obj.set_thumbnail(Some(PathBuf::from("/tmp/thumb.png")));
        assert_eq!(obj.thumbnail(), Some(PathBuf::from("/tmp/thumb.png")));
    }
}
