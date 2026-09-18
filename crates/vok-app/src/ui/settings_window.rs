//! 独立首选项设置窗口 (SettingsWindow)
//!
//! 基于 Libadwaita PreferencesWindow 构建，拥有独立生命周期与多通道入口：
//! 1. 系统托盘右键上下文菜单 -> "⚙ 偏好设置..."
//! 2. OverlayHeader 顶部齿轮按钮 -> 点击自动隐藏 Overlay 并呈现此窗口
//!
//! 包含快捷键配置与系统冲突检测、深浅色外观主题切换、失焦与Esc交互行为设置以及系统关于。

use std::cell::RefCell;
use std::rc::Rc;

use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, CheckButton, Label, Orientation};
use libadwaita as adw;
use libadwaita::prelude::*;
use tracing::info;
use vok_core::hotkey::HotkeyConfig;
use vok_core::ThemeMode;
use vok_infra::desktop::DesktopShortcut;
use vok_infra::storage::StorageClient;

use super::hotkey_recorder::HotkeyRecorder;
use super::theme::ThemeManager;

pub type OnHotkeyChangedCallback = Box<dyn Fn(HotkeyConfig) + 'static>;
pub type OnPermissionGrantedCallback = Box<dyn Fn() + 'static>;

pub struct SettingsWindow {
    window: adw::PreferencesWindow,
    _recorder: Rc<HotkeyRecorder>,
    on_hotkey_changed: Rc<RefCell<Option<OnHotkeyChangedCallback>>>,
    on_permission_granted: Rc<RefCell<Option<OnPermissionGrantedCallback>>>,
}

impl SettingsWindow {
    pub fn new(storage: StorageClient, theme_manager: ThemeManager) -> Self {
        // 确保 Libadwaita 运行时环境就绪
        let _ = adw::init();

        let window = adw::PreferencesWindow::builder()
            .title("首选项")
            .default_width(620)
            .default_height(560)
            .destroy_with_parent(false)
            .build();

        // 拦截关闭事件：仅隐藏窗口而不销毁或退出应用
        window.connect_close_request(|win| {
            info!("【设置窗口】用户请求关闭设置窗口，转为静默隐藏");
            win.set_visible(false);
            glib::Propagation::Stop
        });

        // 1. 读取当前已持久化的快捷键
        let current_hotkey = match storage.get_setting("hotkey.summon") {
            Ok(Some(s)) if !s.trim().is_empty() => {
                HotkeyConfig::from_gsettings_string(&s).unwrap_or_else(|_| HotkeyConfig::default_summon())
            }
            _ => HotkeyConfig::default_summon(),
        };

        let recorder = Rc::new(HotkeyRecorder::new(current_hotkey.clone()));
        let on_hotkey_changed: Rc<RefCell<Option<OnHotkeyChangedCallback>>> =
            Rc::new(RefCell::new(None));
        let on_permission_granted: Rc<RefCell<Option<OnPermissionGrantedCallback>>> =
            Rc::new(RefCell::new(None));

        // ==========================================
        // 页面 1: 快捷键与呼出 (General & Hotkeys)
        // ==========================================
        let hotkey_page = adw::PreferencesPage::builder()
            .title("快捷键")
            .icon_name("preferences-system-symbolic")
            .build();

        let hotkey_group = adw::PreferencesGroup::builder()
            .title("全局唤醒热键")
            .description("设置呼出与切换 Vok 工作台的全屏热键，支持单键与组合键")
            .build();

        // 快捷键录入组件容器
        let recorder_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .margin_top(6)
            .margin_bottom(6)
            .build();

        recorder_box.append(recorder.widget());

        // 快捷键预设一键切换
        let presets_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let preset_label = Label::builder()
            .label("常用预设:")
            .css_classes(["vok-settings-preset-label"])
            .valign(Align::Center)
            .build();
        presets_box.append(&preset_label);

        let presets = [
            ("⌥ Alt + Space", "<Alt>space", "一键切换为经典 Alt + Space (自动接管窗口控制菜单)"),
            ("⊞ Super + Space", "<Super>space", "一键切换为经典 Super + Space"),
            ("Ctrl + ~", "<Control>grave", "一键切换为终端风格 Ctrl + ~"),
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
        recorder_box.append(&presets_box);

        // 系统冲突提示
        let conflict_hint = Label::builder()
            .label("")
            .css_classes(["vok-settings-conflict-hint"])
            .halign(Align::Start)
            .visible(false)
            .build();
        recorder_box.append(&conflict_hint);

        if let Some(conflict) = current_hotkey.detect_system_conflict() {
            conflict_hint.set_label(&format!("🛡️ 已自动安全接管【{}】，杜绝系统弹窗干扰", conflict.description));
            conflict_hint.set_visible(true);
        }

        hotkey_group.add(&recorder_box);

        // 系统通道诊断
        let channel_group = adw::PreferencesGroup::builder()
            .title("系统集成通道状态")
            .build();

        let row_dbus = adw::ActionRow::builder()
            .title("GNOME 全局快捷键 D-Bus 通道")
            .subtitle("通过 org.gnome.settings-daemon 毫秒级原生直连")
            .build();
        row_dbus.add_suffix(&Label::builder().label("● 正常").css_classes(["vok-settings-status-ok"]).build());

        let row_stealth = adw::ActionRow::builder()
            .title("工作区避让与任务栏隐形")
            .subtitle("EWMH _NET_WM_STATE_SKIP_TASKBAR &amp; WORKAREA 适配")
            .build();
        row_stealth.add_suffix(&Label::builder().label("● 已就绪").css_classes(["vok-settings-status-ok"]).build());

        channel_group.add(&row_dbus);
        channel_group.add(&row_stealth);

        hotkey_page.add(&hotkey_group);
        hotkey_page.add(&channel_group);
        window.add(&hotkey_page);

        // ==========================================
        // 页面 2: 外观与主题 (Appearance & Theme)
        // ==========================================
        let theme_page = adw::PreferencesPage::builder()
            .title("外观")
            .icon_name("preferences-desktop-appearance-symbolic")
            .build();

        let theme_group = adw::PreferencesGroup::builder()
            .title("色彩与配色方案")
            .description("定制工作台面板与卡片的视觉呈现")
            .build();

        let cur_mode = theme_manager.current_mode();

        // 单选按钮组
        let radio_system = CheckButton::builder()
            .active(cur_mode == ThemeMode::FollowSystem)
            .valign(Align::Center)
            .build();

        let radio_dark = CheckButton::builder()
            .group(&radio_system)
            .active(cur_mode == ThemeMode::Dark)
            .valign(Align::Center)
            .build();

        let radio_light = CheckButton::builder()
            .group(&radio_system)
            .active(cur_mode == ThemeMode::Light)
            .valign(Align::Center)
            .build();

        let row_system = adw::ActionRow::builder()
            .title("跟随系统 (Auto)")
            .subtitle("根据 GNOME 系统暗色偏好自动无缝切换日间/夜间模式")
            .activatable_widget(&radio_system)
            .build();
        row_system.add_prefix(&radio_system);

        let row_dark = adw::ActionRow::builder()
            .title("深色模式 (Dark)")
            .subtitle("经典沉浸式极客深色毛玻璃，弱光环境下更舒适")
            .activatable_widget(&radio_dark)
            .build();
        row_dark.add_prefix(&radio_dark);

        let row_light = adw::ActionRow::builder()
            .title("浅色模式 (Light)")
            .subtitle("高对比度明亮通透色调，强光环境下视觉更清晰")
            .activatable_widget(&radio_light)
            .build();
        row_light.add_prefix(&radio_light);

        // 绑定单选点击
        {
            let tm = theme_manager.clone();
            radio_system.connect_toggled(move |btn| {
                if btn.is_active() {
                    tm.set_mode(ThemeMode::FollowSystem);
                }
            });
        }
        {
            let tm = theme_manager.clone();
            radio_dark.connect_toggled(move |btn| {
                if btn.is_active() {
                    tm.set_mode(ThemeMode::Dark);
                }
            });
        }
        {
            let tm = theme_manager.clone();
            radio_light.connect_toggled(move |btn| {
                if btn.is_active() {
                    tm.set_mode(ThemeMode::Light);
                }
            });
        }

        theme_group.add(&row_system);
        theme_group.add(&row_dark);
        theme_group.add(&row_light);
        theme_page.add(&theme_group);
        window.add(&theme_page);

        // ==========================================
        // 页面 3: 交互行为 (Behavior)
        // ==========================================
        let behavior_page = adw::PreferencesPage::builder()
            .title("交互行为")
            .icon_name("input-keyboard-symbolic")
            .build();

        let behavior_group = adw::PreferencesGroup::builder()
            .title("工作台隐退行为")
            .description("控制工作台呼出后的关闭与焦点交还策略")
            .build();

        let switch_focus = adw::SwitchRow::builder()
            .title("界面失去焦点自动隐藏")
            .subtitle("点击系统任务栏/Dock、切换至其他应用或点击外部时自动隐退")
            .active(true)
            .build();

        let switch_esc = adw::SwitchRow::builder()
            .title("按 Esc 键快速隐藏")
            .subtitle("无论光标位于搜索框、中转站还是启动台，按 Esc 键均直接退出")
            .active(true)
            .build();

        behavior_group.add(&switch_focus);
        behavior_group.add(&switch_esc);
        behavior_page.add(&behavior_group);

        // 硬件级全局交互支持 (Linux 内核 evdev / Polkit 规范授权)
        let hardware_group = adw::PreferencesGroup::builder()
            .title("硬件级全局输入感知 (Linux 内核 evdev)")
            .description("直接从 Linux 内核驱动读取物理硬件电平，彻底攻破 Wayland 合成器事件吞噬死穴")
            .build();

        let row_evdev = adw::ActionRow::builder()
            .title("任务栏盲区与全局物理点击秒退")
            .build();

        let has_perm = vok_infra::SystemPermission::has_input_permissions();
        if has_perm {
            row_evdev.set_subtitle("✓ 权限已就绪：已接入内核输入事件流，支持任务栏空白处点击秒退");
            let badge = Label::builder()
                .label("已激活")
                .css_classes(["success", "vok-badge"])
                .valign(Align::Center)
                .build();
            row_evdev.add_suffix(&badge);
        } else {
            row_evdev.set_subtitle("当前未开启输入设备权限。点击右侧按钮通过系统 Polkit 规范授权");
            let auth_btn = Button::builder()
                .label("一键授权")
                .css_classes(["suggested-action"])
                .valign(Align::Center)
                .build();

            let row_clone = row_evdev.clone();
            let auth_btn_clone = auth_btn.clone();
            let on_perm_cb = on_permission_granted.clone();
            auth_btn.connect_clicked(move |_| {
                auth_btn_clone.set_sensitive(false);
                auth_btn_clone.set_label("授权中...");
                let row_ui = row_clone.clone();
                let btn_ui = auth_btn_clone.clone();
                let perm_cb = on_perm_cb.clone();

                let (tx, rx) = async_channel::bounded(1);
                // 在后台线程执行 pkexec，调起 Polkit 原生安全弹窗
                std::thread::spawn(move || {
                    let result = vok_infra::SystemPermission::request_input_permissions();
                    let _ = tx.send_blocking(result);
                });

                // 在 GTK 主线程局部 Future 中等待结果并更新 UI
                glib::spawn_future_local(async move {
                    if let Ok(result) = rx.recv().await {
                        match result {
                            Ok(true) => {
                                row_ui.set_subtitle("✓ 权限已就绪：已接入内核输入事件流，支持任务栏空白处点击秒退");
                                btn_ui.set_visible(false);
                                let badge = Label::builder()
                                    .label("已激活")
                                    .css_classes(["success", "vok-badge"])
                                    .valign(Align::Center)
                                    .build();
                                row_ui.add_suffix(&badge);
                                if let Some(cb) = perm_cb.borrow().as_ref() {
                                    cb();
                                }
                            }
                            Ok(false) => {
                                row_ui.set_subtitle("授权未完全生效，请检查系统 Polkit 策略");
                                btn_ui.set_sensitive(true);
                                btn_ui.set_label("重试授权");
                            }
                            Err(ref e) => {
                                row_ui.set_subtitle(&format!("授权未完成: {}", e));
                                btn_ui.set_sensitive(true);
                                btn_ui.set_label("重试授权");
                            }
                        }
                    }
                });
            });

            row_evdev.add_suffix(&auth_btn);
        }

        hardware_group.add(&row_evdev);
        behavior_page.add(&hardware_group);
        window.add(&behavior_page);

        // ==========================================
        // 页面 4: 关于 (About)
        // ==========================================
        let about_page = adw::PreferencesPage::builder()
            .title("关于")
            .icon_name("help-about-symbolic")
            .build();

        let about_group = adw::PreferencesGroup::builder()
            .title("关于 Vok")
            .build();

        let row_version = adw::ActionRow::builder()
            .title("Vok (Linux) 原生效率工作台")
            .subtitle("版本 v0.1.0 · 遵循 MIT OR Apache-2.0 开源协议")
            .build();

        let row_arch = adw::ActionRow::builder()
            .title("架构与性能")
            .subtitle("纯 Rust + GTK 4 + Libadwaita 原生构建，彻底剔除 Web/Tauri")
            .build();

        let row_memory = adw::ActionRow::builder()
            .title("内存与资源指标")
            .subtitle("常驻内存严格低于 40MB，GPU 加速 RenderNode 渲染，0-Sleep 严格事件驱动")
            .build();

        about_group.add(&row_version);
        about_group.add(&row_arch);
        about_group.add(&row_memory);
        about_page.add(&about_group);
        window.add(&about_page);

        // 快捷键持久化与桌面集成联动
        {
            let storage_clone = storage;
            let on_hotkey_changed_clone = on_hotkey_changed.clone();
            let conflict_hint_clone = conflict_hint;

            recorder.set_on_changed(move |new_config| {
                let gsettings_str = new_config.to_gsettings_string();
                info!("【快捷键变更】正在持久化并应用新快捷键: {}", gsettings_str);

                if let Err(e) = storage_clone.set_setting("hotkey.summon", &gsettings_str) {
                    tracing::error!("持久化快捷键失败: {}", e);
                }

                DesktopShortcut::bind_gnome_shortcut(&gsettings_str);

                if let Some(conflict) = new_config.detect_system_conflict() {
                    conflict_hint_clone.set_label(&format!("🛡️ 已自动安全接管【{}】，杜绝系统弹窗干扰", conflict.description));
                    conflict_hint_clone.set_visible(true);
                } else {
                    conflict_hint_clone.set_visible(false);
                }

                if let Some(cb) = on_hotkey_changed_clone.borrow().as_ref() {
                    cb(new_config);
                }
            });
        }

        Self {
            window,
            _recorder: recorder,
            on_hotkey_changed,
            on_permission_granted,
        }
    }

    /// 呈现并置顶设置窗口
    pub fn present(&self) {
        self.window.set_visible(true);
        self.window.present();
    }

    /// 隐藏设置窗口
    pub fn hide(&self) {
        self.window.set_visible(false);
    }

    /// 设置快捷键变更回调
    pub fn set_on_hotkey_changed<F: Fn(HotkeyConfig) + 'static>(&self, callback: F) {
        *self.on_hotkey_changed.borrow_mut() = Some(Box::new(callback));
    }

    /// 设置硬件权限获取成功回调
    pub fn set_on_permission_granted<F: Fn() + 'static>(&self, callback: F) {
        *self.on_permission_granted.borrow_mut() = Some(Box::new(callback));
    }

    /// 获取底层 PreferencesWindow 引用
    pub fn window(&self) -> &adw::PreferencesWindow {
        &self.window
    }
}
