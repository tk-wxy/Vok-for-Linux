//! 主题模式契约定义 (跟随系统 / 浅色 / 深色)

use std::fmt;

/// 主题外观偏好模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// 跟随系统偏好 (GNOME / FreeDesktop Color-Scheme)
    #[default]
    FollowSystem,
    /// 强制深色毛玻璃模式
    Dark,
    /// 强制浅色通透模式
    Light,
}

impl ThemeMode {
    /// 转化为持久化存储字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FollowSystem => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    /// 从存储字符串解析
    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "dark" => Self::Dark,
            "light" => Self::Light,
            _ => Self::FollowSystem,
        }
    }

    /// 用户界面展示文本
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::FollowSystem => "跟随系统 (Auto)",
            Self::Dark => "深色模式 (Dark)",
            Self::Light => "浅色模式 (Light)",
        }
    }
}

impl fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_mode_serialization() {
        assert_eq!(ThemeMode::FollowSystem.as_str(), "system");
        assert_eq!(ThemeMode::Dark.as_str(), "dark");
        assert_eq!(ThemeMode::Light.as_str(), "light");

        assert_eq!(ThemeMode::from_str_loose("system"), ThemeMode::FollowSystem);
        assert_eq!(ThemeMode::from_str_loose("DARK"), ThemeMode::Dark);
        assert_eq!(ThemeMode::from_str_loose("light"), ThemeMode::Light);
        assert_eq!(ThemeMode::from_str_loose("unknown"), ThemeMode::FollowSystem);
    }
}
