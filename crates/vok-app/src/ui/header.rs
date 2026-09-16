use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, SearchEntry};

/// 顶部统一导航条组件
pub struct HeaderComponent {
    container: GtkBox,
    search_entry: SearchEntry,
    #[allow(dead_code)]
    settings_btn: Button,
    status_label: Label,
}

impl Default for HeaderComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl HeaderComponent {
    pub fn new() -> Self {
        let container = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["vok-header"])
            .spacing(16)
            .valign(Align::Center)
            .build();

        // 1. 标志 / 标题
        let title_label = Label::builder()
            .label("VOK")
            .css_classes(["vok-title"])
            .halign(Align::Start)
            .valign(Align::Center)
            .build();

        // 2. 居中全局统一搜索框 (Ctrl+K)
        let search_entry = SearchEntry::builder()
            .placeholder_text("搜索启动项、中转文件与剪贴板 (Ctrl+K)...")
            .css_classes(["vok-search-entry"])
            .hexpand(true)
            .halign(Align::Center)
            .valign(Align::Center)
            .build();

        // 3. 右上角功能区 (设置齿轮与状态提示)
        let right_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .halign(Align::End)
            .valign(Align::Center)
            .build();

        let status_label = Label::builder()
            .label("就绪 - [Esc] 退出")
            .css_classes(["vok-status"])
            .valign(Align::Center)
            .build();

        let settings_btn = Button::builder()
            .label("⚙ 设置")
            .css_classes(["vok-settings-btn"])
            .valign(Align::Center)
            .build();

        right_box.append(&status_label);
        right_box.append(&settings_btn);

        container.append(&title_label);
        container.append(&search_entry);
        container.append(&right_box);

        Self {
            container,
            search_entry,
            settings_btn,
            status_label,
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    pub fn search_entry(&self) -> &SearchEntry {
        &self.search_entry
    }

    #[allow(dead_code)]
    pub fn settings_btn(&self) -> &Button {
        &self.settings_btn
    }

    pub fn set_status(&self, text: &str) {
        self.status_label.set_label(text);
    }
}
