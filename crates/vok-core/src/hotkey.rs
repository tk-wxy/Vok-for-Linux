use std::time::Duration;
use thiserror::Error;

/// 热键触发模式判决阈值 (毫秒)
/// 
/// 按住物理按键超过此时间后松开被视作长按瞬态（Momentary，松手即隐），
/// 小于等于此时间被视作短按翻转（Toggle，松手保持显示）。
pub const DEFAULT_TAP_MAX_MS: u64 = 280;

/// 热键工作模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyMode {
    /// 瞬态长按模式：按下立即揭示面板，松开瞬间立即隐藏（主要用于抓取外部文件临时丢入中转站）
    Momentary,
    /// 保持翻转模式：短按松开后常驻显示；再次按下或按 Esc 隐藏
    Toggle,
}

impl HotkeyMode {
    /// 根据物理按下至释放的持续时间判决行为模式
    pub fn from_duration(duration: Duration, threshold: Duration) -> Self {
        if duration > threshold {
            HotkeyMode::Momentary
        } else {
            HotkeyMode::Toggle
        }
    }
}

/// 快捷键解析与反序列化错误
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HotkeyParseError {
    #[error("快捷键字符串为空")]
    EmptyString,
    #[error("缺少实体按键: {0}")]
    MissingKey(String),
    #[error("未闭合或无效的修饰键标签: {0}")]
    MalformedTag(String),
    #[error("未知的修饰键标签: <{0}>")]
    UnknownModifier(String),
}

/// 修饰键标志位集合
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ModifierMask {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl ModifierMask {
    pub const EMPTY: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        super_key: false,
    };

    pub fn new(ctrl: bool, alt: bool, shift: bool, super_key: bool) -> Self {
        Self {
            ctrl,
            alt,
            shift,
            super_key,
        }
    }

    /// 是否未按下任何修饰键
    pub fn is_empty(&self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.super_key
    }

    /// 转换为 GSettings 规范修饰键字符串列表，如 ["<Super>", "<Control>"]
    pub fn to_gsettings_prefix(&self) -> String {
        let mut s = String::new();
        if self.super_key {
            s.push_str("<Super>");
        }
        if self.ctrl {
            s.push_str("<Control>");
        }
        if self.alt {
            s.push_str("<Alt>");
        }
        if self.shift {
            s.push_str("<Shift>");
        }
        s
    }

    /// 转换为人类易读的修饰键列表前缀，如 ["Super", "Ctrl"]
    pub fn to_display_prefix(&self) -> Vec<&'static str> {
        let mut list = Vec::new();
        if self.super_key {
            list.push("Super");
        }
        if self.ctrl {
            list.push("Ctrl");
        }
        if self.alt {
            list.push("Alt");
        }
        if self.shift {
            list.push("Shift");
        }
        list
    }
}

/// 快捷键领域模型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeyConfig {
    pub modifiers: ModifierMask,
    pub key_name: String,
    pub keyval: u32,
}

impl HotkeyConfig {
    /// 默认全局呼出快捷键 (F12)
    pub fn default_summon() -> Self {
        Self {
            modifiers: ModifierMask::EMPTY,
            key_name: "F12".to_string(),
            keyval: 0xffc9, // GDK_KEY_F12
        }
    }

    /// 创建新的快捷键配置
    pub fn new(modifiers: ModifierMask, key_name: impl Into<String>, keyval: u32) -> Self {
        let key_name = key_name.into();
        let keyval = if keyval == 0 {
            Self::derive_keyval(&key_name)
        } else {
            keyval
        };
        Self {
            modifiers,
            key_name,
            keyval,
        }
    }

    /// 根据键名推导标准 X11/GDK keysym 码
    pub fn derive_keyval(key_name: &str) -> u32 {
        let lower = key_name.to_ascii_lowercase();
        // F1 - F24
        if let Some(stripped) = lower.strip_prefix('f') {
            if let Ok(num) = stripped.parse::<u32>() {
                if (1..=24).contains(&num) {
                    return 0xffbe + (num - 1);
                }
            }
        }
        match lower.as_str() {
            "space" => 0x0020,
            "return" | "enter" => 0xff0d,
            "escape" | "esc" => 0xff1b,
            "backspace" => 0xff08,
            "tab" => 0xff09,
            "grave" | "`" => 0x0060,
            "asciitilde" | "~" => 0x007e,
            s if s.len() == 1 => {
                let c = s.chars().next().unwrap();
                c as u32
            }
            _ => 0,
        }
    }

    /// 转换为 GSettings 格式，如 "<Super>space", "F12", "<Control><Alt>k"
    pub fn to_gsettings_string(&self) -> String {
        format!("{}{}", self.modifiers.to_gsettings_prefix(), self.key_name)
    }

    /// 从 GSettings 格式解析，如 "<Super>space", "F12", "<Control><Alt>k"
    pub fn from_gsettings_string(s: &str) -> Result<Self, HotkeyParseError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(HotkeyParseError::EmptyString);
        }

        let mut modifiers = ModifierMask::EMPTY;
        let mut rest = trimmed;

        while rest.starts_with('<') {
            let close_idx = rest.find('>').ok_or_else(|| {
                HotkeyParseError::MalformedTag(rest.to_string())
            })?;
            let tag = &rest[1..close_idx];
            let tag_lower = tag.to_ascii_lowercase();
            match tag_lower.as_str() {
                "super" | "super_l" | "super_r" | "mod4" => modifiers.super_key = true,
                "control" | "ctrl" | "primary" => modifiers.ctrl = true,
                "alt" | "mod1" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                _ => return Err(HotkeyParseError::UnknownModifier(tag.to_string())),
            }
            rest = &rest[close_idx + 1..];
        }

        let key_name = rest.trim();
        if key_name.is_empty() {
            return Err(HotkeyParseError::MissingKey(trimmed.to_string()));
        }

        let keyval = Self::derive_keyval(key_name);
        Ok(Self {
            modifiers,
            key_name: key_name.to_string(),
            keyval,
        })
    }

    /// 转换为人类易读的展示文本，如 "Super + Space", "Ctrl + Alt + K", "F12"
    pub fn to_display_string(&self) -> String {
        let mut parts = self.modifiers.to_display_prefix();
        let display_key = Self::format_display_key(&self.key_name);
        parts.push(&display_key);
        parts.join(" + ")
    }

    /// 格式化实体键名用于 UI 呈现
    pub fn format_display_key(key_name: &str) -> String {
        let lower = key_name.to_ascii_lowercase();
        if let Some(stripped) = lower.strip_prefix('f') {
            if stripped.chars().all(|c| c.is_ascii_digit()) {
                return key_name.to_ascii_uppercase();
            }
        }
        match lower.as_str() {
            "space" => "Space".to_string(),
            "grave" | "`" => "~".to_string(),
            "return" | "enter" => "Enter".to_string(),
            "escape" | "esc" => "Esc".to_string(),
            "backspace" => "Backspace".to_string(),
            "tab" => "Tab".to_string(),
            s if s.len() == 1 => s.to_ascii_uppercase(),
            _ => {
                // 首字母大写
                let mut c = key_name.chars();
                match c.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                }
            }
        }
    }

    /// 校验快捷键组合是否合法且安全
    ///
    /// 校验规则：
    /// 1. 实体键名不得为空；
    /// 2. 实体键名不能为纯修饰键本身（如 "Control_L" 等）；
    /// 3. 若无修饰键，仅允许功能键 F1~F24；任何普通字符/数字/空格/回车单键一律拒绝；
    /// 4. 若有修饰键，不能只按下 Shift 作为修饰键（避免污染大写输入）；
    /// 5. 必须至少包含 Super、Ctrl、Alt 之一，或者是带修饰键的有效组合。
    pub fn is_valid(&self) -> bool {
        let trimmed = self.key_name.trim();
        if trimmed.is_empty() {
            return false;
        }

        let lower = trimmed.to_ascii_lowercase();
        // 过滤单独修饰键
        if matches!(
            lower.as_str(),
            "control_l"
                | "control_r"
                | "alt_l"
                | "alt_r"
                | "super_l"
                | "super_r"
                | "shift_l"
                | "shift_r"
                | "meta_l"
                | "meta_r"
                | "control"
                | "ctrl"
                | "alt"
                | "super"
                | "shift"
        ) {
            return false;
        }

        let is_fkey = if let Some(stripped) = lower.strip_prefix('f') {
            stripped.parse::<u32>().map(|n| (1..=24).contains(&n)).unwrap_or(false)
        } else {
            false
        };

        // 无修饰键：只有功能键合法
        if self.modifiers.is_empty() {
            return is_fkey;
        }

        // 仅有 Shift 修饰键而无 Super/Ctrl/Alt：对字母/数字是非法的，避免破坏普通输入
        if self.modifiers.shift && !self.modifiers.super_key && !self.modifiers.ctrl && !self.modifiers.alt {
            return false;
        }

        // 必须含有 Super, Ctrl, 或 Alt 之一
        if !self.modifiers.super_key && !self.modifiers.ctrl && !self.modifiers.alt {
            return false;
        }

        // 严禁绑定纯系统导航危险按键（即使带修饰键，单裸 Esc 也应受限）
        if lower == "escape" || lower == "esc" {
            // Esc 即使带修饰键也极易与退出冲突，保留为专用关闭键
            return false;
        }

        true
    }

    /// 检测当前快捷键是否存在已知桌面级系统拦截冲突 (如 GNOME 的 Alt+Space 或 Super+Space)
    pub fn detect_system_conflict(&self) -> Option<SystemConflictInfo> {
        let binding = self.to_gsettings_string();
        match binding.as_str() {
            "<Alt>space" => Some(SystemConflictInfo {
                schema: "org.gnome.desktop.wm.keybindings",
                key: "activate-window-menu",
                description: "系统窗口控制菜单 (弹出最小化/最大化/关闭菜单)",
                original_default: "['<Alt>space']",
            }),
            "<Super>space" => Some(SystemConflictInfo {
                schema: "org.gnome.desktop.wm.keybindings",
                key: "switch-input-source",
                description: "系统输入源/输入法切换浮层",
                original_default: "['<Super>space', 'XF86Keyboard']",
            }),
            _ => None,
        }
    }
}

/// 系统快捷键冲突信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemConflictInfo {
    pub schema: &'static str,
    pub key: &'static str,
    pub description: &'static str,
    pub original_default: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotkey_mode_decision() {
        let threshold = Duration::from_millis(DEFAULT_TAP_MAX_MS);

        // 持续 50ms：短按
        assert_eq!(
            HotkeyMode::from_duration(Duration::from_millis(50), threshold),
            HotkeyMode::Toggle
        );

        // 持续 280ms：边界仍然判定为 Toggle
        assert_eq!(
            HotkeyMode::from_duration(Duration::from_millis(DEFAULT_TAP_MAX_MS), threshold),
            HotkeyMode::Toggle
        );

        // 持续 281ms：长按判定为 Momentary
        assert_eq!(
            HotkeyMode::from_duration(Duration::from_millis(DEFAULT_TAP_MAX_MS + 1), threshold),
            HotkeyMode::Momentary
        );

        // 持续 1000ms：长按 Momentary
        assert_eq!(
            HotkeyMode::from_duration(Duration::from_millis(1000), threshold),
            HotkeyMode::Momentary
        );
    }

    #[test]
    fn test_hotkey_to_from_gsettings_roundtrip() {
        let f12 = HotkeyConfig::default_summon();
        assert_eq!(f12.to_gsettings_string(), "F12");
        assert_eq!(f12.to_display_string(), "F12");
        assert_eq!(HotkeyConfig::from_gsettings_string("F12").unwrap(), f12);

        let super_space = HotkeyConfig::new(
            ModifierMask::new(false, false, false, true),
            "space",
            0,
        );
        assert_eq!(super_space.to_gsettings_string(), "<Super>space");
        assert_eq!(super_space.to_display_string(), "Super + Space");
        assert_eq!(
            HotkeyConfig::from_gsettings_string("<Super>space").unwrap(),
            super_space
        );

        let ctrl_alt_k = HotkeyConfig::new(
            ModifierMask::new(true, true, false, false),
            "k",
            0,
        );
        assert_eq!(ctrl_alt_k.to_gsettings_string(), "<Control><Alt>k");
        assert_eq!(ctrl_alt_k.to_display_string(), "Ctrl + Alt + K");
        assert_eq!(
            HotkeyConfig::from_gsettings_string("<Control><Alt>k").unwrap(),
            ctrl_alt_k
        );

        let primary_alt = HotkeyConfig::from_gsettings_string("<Primary><Alt>k").unwrap();
        assert!(primary_alt.modifiers.ctrl);
        assert!(primary_alt.modifiers.alt);
        assert_eq!(primary_alt.key_name, "k");
    }

    #[test]
    fn test_hotkey_validation() {
        // 单键 F12: 合法
        assert!(HotkeyConfig::default_summon().is_valid());

        // 单键 F1: 合法
        assert!(HotkeyConfig::new(ModifierMask::EMPTY, "F1", 0).is_valid());

        // 单键字母 'a': 非法
        assert!(!HotkeyConfig::new(ModifierMask::EMPTY, "a", 0).is_valid());

        // 单键空格: 非法
        assert!(!HotkeyConfig::new(ModifierMask::EMPTY, "space", 0).is_valid());

        // 单键回车: 非法
        assert!(!HotkeyConfig::new(ModifierMask::EMPTY, "Return", 0).is_valid());

        // Super + Space: 合法
        assert!(HotkeyConfig::new(
            ModifierMask::new(false, false, false, true),
            "space",
            0
        ).is_valid());

        // Ctrl + Alt + K: 合法
        assert!(HotkeyConfig::new(
            ModifierMask::new(true, true, false, false),
            "k",
            0
        ).is_valid());

        // 仅 Shift + A: 非法 (破坏常规输入)
        assert!(!HotkeyConfig::new(
            ModifierMask::new(false, false, true, false),
            "a",
            0
        ).is_valid());

        // Ctrl + Shift + P: 合法 (包含了 Ctrl)
        assert!(HotkeyConfig::new(
            ModifierMask::new(true, false, true, false),
            "p",
            0
        ).is_valid());

        // 单纯修饰键: 非法
        assert!(!HotkeyConfig::new(
            ModifierMask::new(true, false, false, false),
            "Control_L",
            0
        ).is_valid());

        // 包含 Esc: 非法
        assert!(!HotkeyConfig::new(
            ModifierMask::new(false, false, false, true),
            "Escape",
            0
        ).is_valid());
    }

    #[test]
    fn test_hotkey_parse_errors() {
        assert_eq!(
            HotkeyConfig::from_gsettings_string(""),
            Err(HotkeyParseError::EmptyString)
        );
        assert_eq!(
            HotkeyConfig::from_gsettings_string("<Super>"),
            Err(HotkeyParseError::MissingKey("<Super>".to_string()))
        );
        assert!(matches!(
            HotkeyConfig::from_gsettings_string("<Unknown>k"),
            Err(HotkeyParseError::UnknownModifier(_))
        ));
        assert!(matches!(
            HotkeyConfig::from_gsettings_string("<Unclosed"),
            Err(HotkeyParseError::MalformedTag(_))
        ));
    }

    #[test]
    fn test_system_conflict_detection() {
        // Alt + Space 必须准确检测为 activate-window-menu 冲突
        let alt_space = HotkeyConfig::new(
            ModifierMask::new(false, true, false, false),
            "space",
            0,
        );
        let conflict_alt = alt_space.detect_system_conflict().expect("should detect conflict");
        assert_eq!(conflict_alt.schema, "org.gnome.desktop.wm.keybindings");
        assert_eq!(conflict_alt.key, "activate-window-menu");

        // Super + Space 必须准确检测为 switch-input-source 冲突
        let super_space = HotkeyConfig::new(
            ModifierMask::new(false, false, false, true),
            "space",
            0,
        );
        let conflict_super = super_space.detect_system_conflict().expect("should detect conflict");
        assert_eq!(conflict_super.schema, "org.gnome.desktop.wm.keybindings");
        assert_eq!(conflict_super.key, "switch-input-source");

        // F12 / Ctrl + Alt + K 无系统预设冲突
        assert!(HotkeyConfig::default_summon().detect_system_conflict().is_none());
        assert!(HotkeyConfig::new(
            ModifierMask::new(true, true, false, false),
            "k",
            0,
        ).detect_system_conflict().is_none());
    }
}
