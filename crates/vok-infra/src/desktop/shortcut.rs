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

        let _ = Command::new("gsettings")
            .args(["set", schema_prefix, "command", &exe_str])
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_gnome_shortcut_does_not_panic() {
        let _ = DesktopShortcut::ensure_gnome_shortcut("F12");
    }
}
