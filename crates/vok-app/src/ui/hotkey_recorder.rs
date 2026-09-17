use std::cell::RefCell;
use std::rc::Rc;

use gtk4::gdk::{Key, ModifierType};
use gtk4::glib::translate::IntoGlib;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, EventControllerFocus, EventControllerKey, GestureClick, Label,
    Orientation,
};
use tracing::debug;
use vok_core::hotkey::{HotkeyConfig, ModifierMask};
use vok_infra::desktop::DesktopShortcut;

pub type HotkeyChangedCallback = Box<dyn Fn(HotkeyConfig) + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecorderState {
    Idle,
    Recording,
}

/// 可视化快捷键录入交互组件
pub struct HotkeyRecorder {
    container: GtkBox,
    #[allow(dead_code)]
    hotkey_box: GtkBox,
    badges_box: GtkBox,
    hint_label: Label,
    #[allow(dead_code)]
    reset_btn: Button,
    current_config: Rc<RefCell<HotkeyConfig>>,
    #[allow(dead_code)]
    state: Rc<RefCell<RecorderState>>,
    on_changed: Rc<RefCell<Option<HotkeyChangedCallback>>>,
}

impl HotkeyRecorder {
    pub fn new(initial_config: HotkeyConfig) -> Self {
        let current_config = Rc::new(RefCell::new(initial_config));
        let state = Rc::new(RefCell::new(RecorderState::Idle));
        let on_changed: Rc<RefCell<Option<HotkeyChangedCallback>>> = Rc::new(RefCell::new(None));

        // 1. 根容器 (横向排布：录入输入区 + 恢复默认按钮)
        let container = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["vok-hotkey-recorder"])
            .spacing(10)
            .valign(Align::Center)
            .build();

        // 2. 交互录入盒 (点击聚焦并捕获按键)
        let hotkey_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .css_classes(["vok-hotkey-box"])
            .spacing(8)
            .focusable(true)
            .can_focus(true)
            .valign(Align::Center)
            .build();

        // 2.1 快捷键键帽药丸展示区
        let badges_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .valign(Align::Center)
            .build();

        // 2.2 状态/提示小标签
        let hint_label = Label::builder()
            .label("点击录入")
            .css_classes(["vok-hotkey-hint"])
            .valign(Align::Center)
            .build();

        hotkey_box.append(&badges_box);
        hotkey_box.append(&hint_label);

        // 3. 恢复默认按钮
        let reset_btn = Button::builder()
            .label("恢复默认")
            .css_classes(["vok-btn-reset"])
            .valign(Align::Center)
            .tooltip_text("恢复为默认快捷键 (F12)")
            .build();

        container.append(&hotkey_box);
        container.append(&reset_btn);

        // 初始渲染
        Self::render_badges(&badges_box, &current_config.borrow());

        // 4. 事件控制器：点击进入录入态 (主动挂起系统冲突按键)
        let click_gesture = GestureClick::new();
        {
            let state_clone = state.clone();
            let hotkey_box_clone = hotkey_box.clone();
            let hint_clone = hint_label.clone();
            let badges_clone = badges_box.clone();

            click_gesture.connect_pressed(move |_, _, _, _| {
                if *state_clone.borrow() == RecorderState::Idle {
                    *state_clone.borrow_mut() = RecorderState::Recording;
                    hotkey_box_clone.grab_focus();
                    hotkey_box_clone.add_css_class("recording");
                    Self::clear_box(&badges_clone);
                    hint_clone.set_label("⚡ 请按下快捷键组合 (Esc 取消)...");

                    // 预先挂起系统可能拦截的快捷键 (如 Alt+Space 对应的窗口控制菜单)
                    DesktopShortcut::prepare_recording_conflicts();
                }
            });
        }
        hotkey_box.add_controller(click_gesture);

        // 5. 事件控制器：键盘事件监听 (采用 Capture 优先捕获阶段)
        let key_controller = EventControllerKey::new();
        key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        {
            let state_clone = state.clone();
            let current_config_clone = current_config.clone();
            let hotkey_box_clone = hotkey_box.clone();
            let hint_clone = hint_label.clone();
            let badges_clone = badges_box.clone();
            let on_changed_clone = on_changed.clone();

            key_controller.connect_key_pressed(move |_, keyval, _keycode, modifier_state| {
                if *state_clone.borrow() != RecorderState::Recording {
                    return glib::Propagation::Proceed;
                }

                // 5.1 按下 Esc 键：取消录入并还原
                if keyval == Key::Escape {
                    debug!("用户按 Esc 取消快捷键录入");
                    *state_clone.borrow_mut() = RecorderState::Idle;
                    hotkey_box_clone.remove_css_class("recording");
                    hint_clone.set_label("点击录入");
                    let current = current_config_clone.borrow().clone();
                    Self::render_badges(&badges_clone, &current);
                    DesktopShortcut::restore_recording_conflicts(&current.to_gsettings_string());
                    return glib::Propagation::Stop;
                }

                // 5.2 判断是否为纯修饰键
                let is_modifier_key = matches!(
                    keyval,
                    Key::Control_L
                        | Key::Control_R
                        | Key::Alt_L
                        | Key::Alt_R
                        | Key::Super_L
                        | Key::Super_R
                        | Key::Shift_L
                        | Key::Shift_R
                        | Key::Meta_L
                        | Key::Meta_R
                );

                if is_modifier_key {
                    // 实时展示按下的修饰键前缀 (兼顾 ALT_MASK 与 META_MASK)
                    let is_ctrl = modifier_state.contains(ModifierType::CONTROL_MASK)
                        || matches!(keyval, Key::Control_L | Key::Control_R);
                    let is_alt = modifier_state.contains(ModifierType::ALT_MASK)
                        || modifier_state.contains(ModifierType::META_MASK)
                        || matches!(keyval, Key::Alt_L | Key::Alt_R | Key::Meta_L | Key::Meta_R);
                    let is_shift = modifier_state.contains(ModifierType::SHIFT_MASK)
                        || matches!(keyval, Key::Shift_L | Key::Shift_R);
                    let is_super = modifier_state.contains(ModifierType::SUPER_MASK)
                        || modifier_state.contains(ModifierType::HYPER_MASK)
                        || matches!(keyval, Key::Super_L | Key::Super_R);

                    let composing_mask = ModifierMask::new(is_ctrl, is_alt, is_shift, is_super);
                    Self::render_composing(&badges_clone, &composing_mask);
                    hint_clone.set_label("等待实体键...");
                    return glib::Propagation::Stop;
                }

                // 5.3 实体键被按下：计算组合键候选
                let ctrl = modifier_state.contains(ModifierType::CONTROL_MASK);
                let alt = modifier_state.contains(ModifierType::ALT_MASK)
                    || modifier_state.contains(ModifierType::META_MASK);
                let shift = modifier_state.contains(ModifierType::SHIFT_MASK);
                let super_key = modifier_state.contains(ModifierType::SUPER_MASK)
                    || modifier_state.contains(ModifierType::HYPER_MASK);

                let key_name = if let Some(name) = keyval.name() {
                    name.to_string()
                } else {
                    format!("{keyval:?}")
                };

                let candidate = HotkeyConfig::new(
                    ModifierMask::new(ctrl, alt, shift, super_key),
                    key_name,
                    keyval.into_glib(),
                );

                // 5.4 校验组合键合法性
                if candidate.is_valid() {
                    *current_config_clone.borrow_mut() = candidate.clone();
                    *state_clone.borrow_mut() = RecorderState::Idle;
                    hotkey_box_clone.remove_css_class("recording");
                    Self::render_badges(&badges_clone, &candidate);
                    hint_clone.set_label("✔ 已生效");

                    // 提交并同步冲突接管状态
                    DesktopShortcut::restore_recording_conflicts(&candidate.to_gsettings_string());

                    if let Some(ref cb) = *on_changed_clone.borrow() {
                        cb(candidate);
                    }
                } else {
                    // 不合法提示
                    hint_clone.set_label("⚠ 普通键需搭配 Super / Ctrl / Alt");
                }

                glib::Propagation::Stop
            });
        }
        hotkey_box.add_controller(key_controller);

        // 6. 事件控制器：失焦自动退出录入态并回滚
        let focus_controller = EventControllerFocus::new();
        {
            let state_clone = state.clone();
            let current_config_clone = current_config.clone();
            let hotkey_box_clone = hotkey_box.clone();
            let hint_clone = hint_label.clone();
            let badges_clone = badges_box.clone();

            focus_controller.connect_leave(move |_| {
                if *state_clone.borrow() == RecorderState::Recording {
                    *state_clone.borrow_mut() = RecorderState::Idle;
                    hotkey_box_clone.remove_css_class("recording");
                    hint_clone.set_label("点击录入");
                    let current = current_config_clone.borrow().clone();
                    Self::render_badges(&badges_clone, &current);
                    DesktopShortcut::restore_recording_conflicts(&current.to_gsettings_string());
                }
            });
        }
        hotkey_box.add_controller(focus_controller);

        // 7. 绑定“恢复默认”按钮
        {
            let current_config_clone = current_config.clone();
            let state_clone = state.clone();
            let hotkey_box_clone = hotkey_box.clone();
            let hint_clone = hint_label.clone();
            let badges_clone = badges_box.clone();
            let on_changed_clone = on_changed.clone();

            reset_btn.connect_clicked(move |_| {
                let def = HotkeyConfig::default_summon();
                *current_config_clone.borrow_mut() = def.clone();
                *state_clone.borrow_mut() = RecorderState::Idle;
                hotkey_box_clone.remove_css_class("recording");
                hint_clone.set_label("已恢复默认");
                Self::render_badges(&badges_clone, &def);
                DesktopShortcut::restore_recording_conflicts(&def.to_gsettings_string());

                if let Some(ref cb) = *on_changed_clone.borrow() {
                    cb(def);
                }
            });
        }

        Self {
            container,
            hotkey_box,
            badges_box,
            hint_label,
            reset_btn,
            current_config,
            state,
            on_changed,
        }
    }

    /// 获取根容器 Widget
    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    /// 设置快捷键变更回调
    pub fn set_on_changed<F: Fn(HotkeyConfig) + 'static>(&self, callback: F) {
        *self.on_changed.borrow_mut() = Some(Box::new(callback));
    }

    /// 获取当前配置
    pub fn config(&self) -> HotkeyConfig {
        self.current_config.borrow().clone()
    }

    /// 外部更新当前快捷键展示
    pub fn update_config(&self, config: HotkeyConfig) {
        *self.current_config.borrow_mut() = config.clone();
        Self::render_badges(&self.badges_box, &config);
        self.hint_label.set_label("点击录入");
    }

    /// 外部直接触发快捷键应用 (如点击预设推荐按钮)
    pub fn trigger_external_change(&self, config: HotkeyConfig) {
        *self.current_config.borrow_mut() = config.clone();
        *self.state.borrow_mut() = RecorderState::Idle;
        self.hotkey_box.remove_css_class("recording");
        Self::render_badges(&self.badges_box, &config);
        self.hint_label.set_label("✔ 已生效");
        DesktopShortcut::restore_recording_conflicts(&config.to_gsettings_string());

        if let Some(ref cb) = *self.on_changed.borrow() {
            cb(config);
        }
    }

    fn clear_box(b: &GtkBox) {
        while let Some(child) = b.first_child() {
            b.remove(&child);
        }
    }

    fn render_badges(badges_box: &GtkBox, config: &HotkeyConfig) {
        Self::clear_box(badges_box);
        let display_str = config.to_display_string();
        let parts: Vec<&str> = display_str.split(" + ").collect();

        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                let plus = Label::builder()
                    .label("+")
                    .css_classes(["vok-key-plus"])
                    .valign(Align::Center)
                    .build();
                badges_box.append(&plus);
            }

            let text = if *part == "Super" {
                "⊞ Super"
            } else {
                part
            };

            let badge = Label::builder()
                .label(text)
                .css_classes(["vok-key-badge"])
                .valign(Align::Center)
                .build();
            badges_box.append(&badge);
        }
    }

    fn render_composing(badges_box: &GtkBox, mask: &ModifierMask) {
        Self::clear_box(badges_box);
        let prefixes = mask.to_display_prefix();
        for (i, p) in prefixes.iter().enumerate() {
            if i > 0 {
                let plus = Label::builder()
                    .label("+")
                    .css_classes(["vok-key-plus"])
                    .valign(Align::Center)
                    .build();
                badges_box.append(&plus);
            }

            let text = if *p == "Super" {
                "⊞ Super"
            } else {
                p
            };

            let badge = Label::builder()
                .label(text)
                .css_classes(["vok-key-badge", "composing"])
                .valign(Align::Center)
                .build();
            badges_box.append(&badge);
        }

        if !prefixes.is_empty() {
            let plus = Label::builder()
                .label("+ ...")
                .css_classes(["vok-key-plus"])
                .valign(Align::Center)
                .build();
            badges_box.append(&plus);
        }
    }
}
