//! 桌面环境全局快捷键自动注册辅助模块 (GNOME / XDG 桌面环境)

use std::process::Command;
use tracing::info;

pub struct DesktopShortcut;

impl DesktopShortcut {
    /// 确保在 GNOME 桌面环境下将指定快捷键（如 "F12"）绑定为执行当前可执行文件
    pub fn ensure_gnome_shortcut(binding: &str) -> bool {
        let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if !current_desktop.contains("GNOME") && !current_desktop.contains("ubuntu") {
            return false;
        }

        let exe_path = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return false,
        };

        let exe_str = exe_path.to_string_lossy();
        let schema_prefix = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/vok/";

        // 1. 设置自定义快捷键属性
        let _ = Command::new("gsettings")
            .args(["set", schema_prefix, "name", "Vok Workbench"])
            .output();

        let trigger_cmd = format!(
            "sh -c \"gdbus call --session --dest com.vok.desktop --object-path /com/vok/desktop --method org.gtk.Actions.Activate 'toggle' [] {{}} >/dev/null 2>&1 || {}\"",
            exe_str
        );

        let _ = Command::new("gsettings")
            .args(["set", schema_prefix, "command", &trigger_cmd])
            .output();

        let _ = Command::new("gsettings")
            .args(["set", schema_prefix, "binding", binding])
            .output();

        // 2. 读取现有 custom-keybindings 列表，确保包含 /vok/
        let output = match Command::new("gsettings")
            .args([
                "get",
                "org.gnome.settings-daemon.plugins.media-keys",
                "custom-keybindings",
            ])
            .output()
        {
            Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
            Err(_) => return false,
        };

        let vok_path = "'/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/vok/'";
        if !output.contains("/vok/") {
            let trimmed = output.trim();
            let new_bindings = if trimmed == "@as []" || trimmed == "[]" {
                format!("[{vok_path}]")
            } else {
                let inner = trimmed.trim_start_matches('[').trim_end_matches(']');
                format!("[{inner}, {vok_path}]")
            };

            let _ = Command::new("gsettings")
                .args([
                    "set",
                    "org.gnome.settings-daemon.plugins.media-keys",
                    "custom-keybindings",
                    &new_bindings,
                ])
                .output();
        }

        info!(
            "【全局快捷键就绪】GNOME 系统级快捷键 [{}] 已关联到 {}",
            binding, exe_str
        );
        true
    }

    /// 确保在 XDG 应用程序目录注册具有 NoDisplay=true 的 Desktop Entry
    /// 使 GNOME Shell / Ubuntu Dock 识别为静默后台应用，避免在 Dock 中显示
    pub fn ensure_desktop_entry() -> bool {
        let home = match std::env::var("HOME") {
            Ok(h) => h,
            Err(_) => return false,
        };
        let app_dir = std::path::PathBuf::from(home).join(".local/share/applications");
        if let Err(e) = std::fs::create_dir_all(&app_dir) {
            tracing::warn!("无法创建 applications 目录: {:?}", e);
            return false;
        }

        let exe_path = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return false,
        };

        let desktop_file = app_dir.join("com.vok.desktop.desktop");
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=Vok\nComment=Linux 原生全屏沉浸式工作台\nExec={}\nIcon=view-grid-symbolic\nTerminal=false\nCategories=Utility;\nNoDisplay=true\n",
            exe_path.to_string_lossy()
        );

        let _ = std::fs::write(&desktop_file, content);
        let _ = Command::new("update-desktop-database").arg(&app_dir).output();
        info!("【静默桌面配置就绪】已安装 com.vok.desktop (NoDisplay=true)");
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_gnome_shortcut_does_not_panic() {
        let _ = DesktopShortcut::ensure_gnome_shortcut("F12");
    }
}
