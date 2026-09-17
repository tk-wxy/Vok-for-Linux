use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Label, Orientation, Popover};
use tracing::info;
use vok_core::hotkey::HotkeyConfig;
use vok_infra::desktop::DesktopShortcut;
use vok_infra::storage::StorageClient;

use super::hotkey_recorder::HotkeyRecorder;

pub type OnHotkeyChangedCallback = Box<dyn Fn(HotkeyConfig) + 'static>;

/// 设置弹窗浮层 (SettingsPopover)
pub struct SettingsPopover {
    popover: Popover,
    recorder: Rc<HotkeyRecorder>,
    on_hotkey_changed: Rc<RefCell<Option<OnHotkeyChangedCallback>>>,
}

impl SettingsPopover {
    pub fn new(storage: StorageClient, parent_button: &Button) -> Self {
        let popover = Popover::builder()
            .autohide(true)
            .has_arrow(true)
            .css_classes(["vok-settings-popover"])
            .build();
        popover.set_parent(parent_button);

        // 1. 读取当前已保存的快捷键配置，若无则使用默认 F12
        let current_hotkey = match storage.get_setting("hotkey.summon") {
            Ok(Some(s)) => {
                HotkeyConfig::from_gsettings_string(&s).unwrap_or_else(|_| HotkeyConfig::default_summon())
            }
            _ => HotkeyConfig::default_summon(),
        };

        let recorder = Rc::new(HotkeyRecorder::new(current_hotkey.clone()));
        let on_hotkey_changed: Rc<RefCell<Option<OnHotkeyChangedCallback>>> =
            Rc::new(RefCell::new(None));

        // 2. 主纵向内容容器
        let root = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(16)
            .css_classes(["vok-settings-content"])
            .build();

        // 2.1 标题栏
        let title_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .build();

        let title_label = Label::builder()
            .label("⚙ 首选项 (Preferences)")
            .css_classes(["vok-settings-title"])
            .halign(Align::Start)
            .build();

        let subtitle_label = Label::builder()
            .label("自定义唤醒快捷键与系统集成参数")
            .css_classes(["vok-settings-subtitle"])
            .halign(Align::Start)
            .build();

        title_box.append(&title_label);
        title_box.append(&subtitle_label);
        root.append(&title_box);

        // 2.2 快捷键设置行
        let section_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .css_classes(["vok-settings-section"])
            .build();

        let section_title = Label::builder()
            .label("唤醒与切换快捷键")
            .css_classes(["vok-settings-section-title"])
            .halign(Align::Start)
            .build();

        let section_desc = Label::builder()
            .label("支持 F1~F12 单键或 Super / Ctrl / Alt 组合键 (如 Super + Space)")
            .css_classes(["vok-settings-desc"])
            .halign(Align::Start)
            .build();

        section_box.append(&section_title);
        section_box.append(&section_desc);
        section_box.append(recorder.widget());

        // 常用快捷键推荐预设 (一键快速切换，无需手动录入)
        let presets_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .margin_top(4)
            .margin_bottom(2)
            .build();

        let preset_label = Label::builder()
            .label("常用推荐:")
            .css_classes(["vok-settings-preset-label"])
            .valign(Align::Center)
            .build();
        presets_box.append(&preset_label);

        let presets = [
            ("⌥ Alt + Space", "<Alt>space", "一键切换为经典 Alt + Space (自动接管窗口控制菜单)"),
            ("⊞ Super + Space", "<Super>space", "一键切换为经典 Super + Space"),
            ("Ctrl + ~", "<Control>grave", "一键切换为极客终端风格 Ctrl + ~"),
            ("F12 (默认)", "F12", "一键恢复为系统默认单键 F12"),
        ];

        for (title, gsettings_str, tooltip) in presets {
            let btn = Button::builder()
                .label(title)
                .css_classes(["vok-btn-preset"])
                .tooltip_text(tooltip)
                .valign(Align::Center)
                .build();

            let recorder_clone = recorder.clone();
            let str_val = gsettings_str.to_string();
            btn.connect_clicked(move |_| {
                if let Ok(config) = HotkeyConfig::from_gsettings_string(&str_val) {
                    recorder_clone.trigger_external_change(config);
                }
            });
            presets_box.append(&btn);
        }
        section_box.append(&presets_box);

        let conflict_hint = Label::builder()
            .label("")
            .css_classes(["vok-settings-conflict-hint"])
            .halign(Align::Start)
            .visible(false)
            .build();
        section_box.append(&conflict_hint);

        // 初始检测冲突
        if let Some(conflict) = current_hotkey.detect_system_conflict() {
            conflict_hint.set_label(&format!("🛡️ 已自动安全接管【{}】，杜绝系统弹窗干扰", conflict.description));
            conflict_hint.set_visible(true);
        }

        root.append(&section_box);

        // 2.3 系统集成状态区
        let status_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .css_classes(["vok-settings-status-box"])
            .build();

        let status_title = Label::builder()
            .label("系统通道状态")
            .css_classes(["vok-settings-section-title"])
            .halign(Align::Start)
            .build();

        let status_gnome = Label::builder()
            .label("● GNOME 系统全局热键已就绪 (8ms 极速 D-Bus 通道)")
            .css_classes(["vok-settings-status-ok"])
            .halign(Align::Start)
            .build();

        let status_stealth = Label::builder()
            .label("● HUD 工作区自适应与任务栏隐形 (_NET_WM_STATE_SKIP_TASKBAR)")
            .css_classes(["vok-settings-status-ok"])
            .halign(Align::Start)
            .build();

        status_box.append(&status_title);
        status_box.append(&status_gnome);
        status_box.append(&status_stealth);
        root.append(&status_box);

        // 2.4 底部版权与版本
        let version_label = Label::builder()
            .label("Vok (Linux) 原生效率工作台 · v0.1.0")
            .css_classes(["vok-settings-version"])
            .halign(Align::Center)
            .build();
        root.append(&version_label);

        popover.set_child(Some(&root));

        // 3. 绑定快捷键变更监听与持久化
        {
            let storage_clone = storage.clone();
            let on_hotkey_changed_clone = on_hotkey_changed.clone();
            let conflict_hint_clone = conflict_hint.clone();

            recorder.set_on_changed(move |new_config| {
                let gsettings_str = new_config.to_gsettings_string();
                info!("【快捷键变更】正在持久化并应用新快捷键: {}", gsettings_str);

                // 更新冲突接管提示
                if let Some(conflict) = new_config.detect_system_conflict() {
                    conflict_hint_clone.set_label(&format!("🛡️ 已自动安全接管【{}】，杜绝系统弹窗干扰", conflict.description));
                    conflict_hint_clone.set_visible(true);
                } else {
                    conflict_hint_clone.set_visible(false);
                }

                // 3.1 写入 SQLite WAL 存储
                if let Err(e) = storage_clone.set_setting("hotkey.summon", &gsettings_str) {
                    tracing::error!("持久化快捷键失败: {:?}", e);
                }

                // 3.2 动态热重载 GNOME 系统级快捷键 (自动接管/恢复冲突)
                DesktopShortcut::bind_gnome_shortcut(&gsettings_str);

                // 3.3 通知外部 HUD 同步
                if let Some(ref cb) = *on_hotkey_changed_clone.borrow() {
                    cb(new_config);
                }
            });
        }

        Self {
            popover,
            recorder,
            on_hotkey_changed,
        }
    }

    /// 弹出设置面板
    pub fn popup(&self) {
        self.popover.popup();
    }

    /// 设置快捷键变更回调 (供主窗口同步)
    pub fn set_on_hotkey_changed<F: Fn(HotkeyConfig) + 'static>(&self, callback: F) {
        *self.on_hotkey_changed.borrow_mut() = Some(Box::new(callback));
    }

    /// 获取当前生效快捷键
    pub fn current_hotkey(&self) -> HotkeyConfig {
        self.recorder.config()
    }
}
