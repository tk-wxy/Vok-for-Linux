use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::error::DesktopError;

/// Linux 标准 XDG 桌面项结构 (.desktop)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    /// 应用显示名称 (优先匹配系统语言)
    pub name: String,
    /// 清洗后的执行命令行 (已剔除 %u, %f 等字段代码)
    pub exec: String,
    /// XDG 主题图标名或图标文件绝对路径
    pub icon: String,
    /// 是否在终端中运行
    pub terminal: bool,
    /// 是否在应用菜单中隐藏 (NoDisplay=true)
    pub no_display: bool,
    /// 应用描述/提示信息
    pub comment: Option<String>,
    /// 应用分类列表
    pub categories: Vec<String>,
}

impl DesktopEntry {
    /// 从文件路径解析 .desktop 文件
    pub fn from_file(path: &Path) -> Result<Self, DesktopError> {
        let content = fs::read_to_string(path)?;
        let lang = env::var("LC_ALL")
            .or_else(|_| env::var("LC_MESSAGES"))
            .or_else(|_| env::var("LANG"))
            .ok();
        Self::parse(&content, lang.as_deref())
    }

    /// 从 .desktop 文本内容解析
    pub fn parse(content: &str, target_locale: Option<&str>) -> Result<Self, DesktopError> {
        let (full_locale, lang_prefix) = if let Some(loc) = target_locale {
            // 如 "zh_CN.UTF-8" -> full: "zh_CN", prefix: "zh"
            let loc_clean = loc.split('.').next().unwrap_or(loc);
            let prefix = loc_clean.split('_').next().unwrap_or(loc_clean);
            (Some(loc_clean), Some(prefix))
        } else {
            (None, None)
        };

        let mut in_desktop_entry_section = false;
        let mut default_name = String::new();
        let mut localized_full_name = None;
        let mut localized_prefix_name = None;

        let mut exec = String::new();
        let mut icon = String::new();
        let mut terminal = false;
        let mut no_display = false;
        let mut comment = None;
        let mut categories = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                let section = &line[1..line.len() - 1];
                in_desktop_entry_section = section == "Desktop Entry";
                continue;
            }

            if !in_desktop_entry_section {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                if key == "Name" {
                    default_name = value.to_string();
                } else if key.starts_with("Name[") && key.ends_with(']') {
                    let loc_key = &key[5..key.len() - 1];
                    if let Some(fl) = full_locale {
                        if loc_key == fl {
                            localized_full_name = Some(value.to_string());
                        }
                    }
                    if let Some(lp) = lang_prefix {
                        if loc_key == lp {
                            localized_prefix_name = Some(value.to_string());
                        }
                    }
                } else if key == "Exec" {
                    exec = Self::clean_exec(value);
                } else if key == "Icon" {
                    icon = value.to_string();
                } else if key == "Terminal" {
                    terminal = value.eq_ignore_ascii_case("true");
                } else if key == "NoDisplay" {
                    no_display = value.eq_ignore_ascii_case("true");
                } else if key == "Comment" {
                    comment = Some(value.to_string());
                } else if key == "Categories" {
                    categories = value
                        .split(';')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
        }

        let name = localized_full_name
            .or(localized_prefix_name)
            .unwrap_or(default_name);

        if name.is_empty() && exec.is_empty() {
            return Err(DesktopError::ParseError(
                "缺少必需的 Name 或 Exec 属性".to_string(),
            ));
        }

        Ok(Self {
            name,
            exec,
            icon,
            terminal,
            no_display,
            comment,
            categories,
        })
    }

    /// 清洗 Exec 字段：去除 %f, %F, %u, %U 等 freedesktop 字段占位符
    pub fn clean_exec(exec_str: &str) -> String {
        let mut cleaned = String::with_capacity(exec_str.len());
        for part in exec_str.split_whitespace() {
            match part {
                "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%i" | "%c" | "%k"
                | "%v" => {}
                _ => {
                    if !cleaned.is_empty() {
                        cleaned.push(' ');
                    }
                    cleaned.push_str(part);
                }
            }
        }
        cleaned
    }

    /// 扫描系统常见的应用程序目录中的 .desktop 文件路径
    pub fn scan_system_applications() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let app_dirs = [
            PathBuf::from("/usr/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            super::super::storage::xdg::get_vok_data_dir()
                .parent()
                .map(|p| p.join("applications"))
                .unwrap_or_else(|| PathBuf::from("/nonexistent")),
        ];

        for dir in &app_dirs {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|ext| ext.to_str()) == Some("desktop") {
                        paths.push(path);
                    }
                }
            }
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_desktop_entry() {
        let content = r#"
[Desktop Entry]
Version=1.0
Type=Application
Name=Firefox Web Browser
Name[zh_CN]=Firefox 网页浏览器
Name[zh]=火狐浏览器
Comment=Browse the World Wide Web
Exec=firefox %u
Icon=firefox
Terminal=false
Categories=Network;WebBrowser;
"#;

        // 1. 指定 zh_CN 本地化语言
        let entry_zh = DesktopEntry::parse(content, Some("zh_CN.UTF-8")).unwrap();
        assert_eq!(entry_zh.name, "Firefox 网页浏览器");
        assert_eq!(entry_zh.exec, "firefox"); // %u 已被清洗
        assert_eq!(entry_zh.icon, "firefox");
        assert!(!entry_zh.terminal);
        assert!(!entry_zh.no_display);
        assert_eq!(entry_zh.categories, vec!["Network", "WebBrowser"]);

        // 2. 指定 zh 本地化语言
        let entry_zh_short = DesktopEntry::parse(content, Some("zh")).unwrap();
        assert_eq!(entry_zh_short.name, "火狐浏览器");

        // 3. 无本地化匹配，回退默认英文
        let entry_en = DesktopEntry::parse(content, Some("en_US.UTF-8")).unwrap();
        assert_eq!(entry_en.name, "Firefox Web Browser");
    }

    #[test]
    fn test_clean_exec_parameters() {
        assert_eq!(
            DesktopEntry::clean_exec("google-chrome-stable --incognito %U"),
            "google-chrome-stable --incognito"
        );
        assert_eq!(
            DesktopEntry::clean_exec("/usr/bin/code --new-window %F"),
            "/usr/bin/code --new-window"
        );
        assert_eq!(DesktopEntry::clean_exec("alacritty"), "alacritty");
    }

    #[test]
    fn test_parse_nodisplay_entry() {
        let content = r#"
[Desktop Entry]
Type=Application
Name=Secret Service
Exec=/usr/bin/secret
Icon=security-high
NoDisplay=true
"#;
        let entry = DesktopEntry::parse(content, None).unwrap();
        assert!(entry.no_display);
    }
}
