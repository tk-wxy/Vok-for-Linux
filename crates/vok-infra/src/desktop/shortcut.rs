//! 桌面环境全局快捷键自动注册辅助模块 (GNOME / XDG 桌面环境)

use std::process::Command;
use tracing::info;

pub struct DesktopShortcut;

impl DesktopShortcut {
    /// 将指定快捷键（如 "<Super>space" 或 "F12"）绑定并热生效到 GNOME 桌面环境
    pub fn bind_gnome_shortcut(binding: &str) -> bool {
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

        // 幂等性检查：若当前绑定已经与目标完全一致，则无需重复重载 gsettings
        if let Some(cur) = Self::read_current_gnome_shortcut() {
            if cur == binding {
                Self::handle_system_conflicts(binding);
                return true;
            }
        }

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

        // 3. 处理潜在系统级快捷键冲突 (如 Alt+Space / Super+Space)
        Self::handle_system_conflicts(binding);

        info!(
            "【全局快捷键热重载】GNOME 系统级快捷键 [{}] 已关联到 {}",
            binding, exe_str
        );
        true
    }

    /// 安全接管或还原可能存在冲突的系统快捷键
    ///
    /// 例如当 binding 为 "<Alt>space" 时，Mutter 默认会弹出窗口操作菜单 (activate-window-menu)。
    /// 将该冲突项设为 "[]" 即可彻底消除弹出菜单的干扰。
    /// 当 binding 变为其他值（如 "F12"）时，自动恢复系统的默认值。
    pub fn handle_system_conflicts(binding: &str) {
        let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if !current_desktop.contains("GNOME") && !current_desktop.contains("ubuntu") {
            return;
        }

        // 1. 处理 Alt + Space 冲突 (activate-window-menu)
        if binding == "<Alt>space" {
            if let Ok(output) = Command::new("gsettings")
                .args([
                    "get",
                    "org.gnome.desktop.wm.keybindings",
                    "activate-window-menu",
                ])
                .output()
            {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if text != "@as []" && text != "[]" {
                    info!("【快捷键冲突安全接管】检测到 Alt+Space 与系统窗口控制菜单冲突，已自动接管并禁用 activate-window-menu");
                    let _ = Command::new("gsettings")
                        .args([
                            "set",
                            "org.gnome.desktop.wm.keybindings",
                            "activate-window-menu",
                            "[]",
                        ])
                        .output();
                }
            }
        } else {
            // 如果之前可能被接管禁用了，检查当前是否为空，若为空且当前不是 Alt+Space，则恢复默认
            if let Ok(output) = Command::new("gsettings")
                .args([
                    "get",
                    "org.gnome.desktop.wm.keybindings",
                    "activate-window-menu",
                ])
                .output()
            {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if text == "@as []" || text == "[]" {
                    info!("【快捷键冲突还原】快捷键已切离 Alt+Space，自动恢复系统 activate-window-menu 为 ['<Alt>space']");
                    let _ = Command::new("gsettings")
                        .args([
                            "set",
                            "org.gnome.desktop.wm.keybindings",
                            "activate-window-menu",
                            "['<Alt>space']",
                        ])
                        .output();
                }
            }
        }

        // 2. 处理 Super + Space 冲突 (switch-input-source)
        if binding == "<Super>space" {
            info!("【快捷键冲突安全接管】检测到 Super+Space 与系统输入源切换冲突，已自动安全避让");
            if let Ok(output) = Command::new("gsettings")
                .args([
                    "get",
                    "org.gnome.desktop.wm.keybindings",
                    "switch-input-source",
                ])
                .output()
            {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if text.contains("<Super>space") {
                    let _ = Command::new("gsettings")
                        .args([
                            "set",
                            "org.gnome.desktop.wm.keybindings",
                            "switch-input-source",
                            "['XF86Keyboard']",
                        ])
                        .output();
                }
            }
        } else if let Ok(output) = Command::new("gsettings")
            .args([
                "get",
                "org.gnome.desktop.wm.keybindings",
                "switch-input-source",
            ])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.contains("<Super>space") && text.contains("XF86Keyboard") {
                info!("【快捷键冲突还原】快捷键已切离 Super+Space，自动恢复系统 switch-input-source 为标准配置");
                let _ = Command::new("gsettings")
                    .args([
                        "set",
                        "org.gnome.desktop.wm.keybindings",
                        "switch-input-source",
                        "['<Super>space', 'XF86Keyboard']",
                    ])
                    .output();
            }
        }
    }

    /// 在进入录入态时，临时挂起已知会拦截按键的系统 WM 快捷键（如 Alt+Space 对应的 activate-window-menu）
    /// 确保物理键盘事件能够直接传达至应用内录入控件，不会被 Mutter 提前拦截并弹出窗口控制菜单
    pub fn prepare_recording_conflicts() {
        let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if !current_desktop.contains("GNOME") && !current_desktop.contains("ubuntu") {
            return;
        }

        if let Ok(output) = Command::new("gsettings")
            .args([
                "get",
                "org.gnome.desktop.wm.keybindings",
                "activate-window-menu",
            ])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if text != "@as []" && text != "[]" {
                info!("【快捷键录入避让】已临时挂起 GNOME 窗口菜单 (activate-window-menu)，确保按键直通录入组件");
                let _ = Command::new("gsettings")
                    .args([
                        "set",
                        "org.gnome.desktop.wm.keybindings",
                        "activate-window-menu",
                        "[]",
                    ])
                    .output();
            }
        }
    }

    /// 在退出录入态（提交、取消或失焦）时，根据当前实际生效的快捷键决定是否恢复系统默认设置
    pub fn restore_recording_conflicts(current_binding: &str) {
        Self::handle_system_conflicts(current_binding);
    }

    /// 保持向后兼容性别名
    pub fn ensure_gnome_shortcut(binding: &str) -> bool {
        Self::bind_gnome_shortcut(binding)
    }

    /// 读取 GNOME 系统当前针对 Vok 绑定的快捷键字符串
    pub fn read_current_gnome_shortcut() -> Option<String> {
        let schema_prefix = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/vok/";
        let output = Command::new("gsettings")
            .args(["get", schema_prefix, "binding"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let trimmed = raw.trim_matches('\'').trim_matches('"').to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    /// 解绑并清空 GNOME 系统快捷键
    pub fn unbind_gnome_shortcut() -> bool {
        Self::handle_system_conflicts("");
        let schema_prefix = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/vok/";
        let status = Command::new("gsettings")
            .args(["set", schema_prefix, "binding", ""])
            .status();
        status.map(|s| s.success()).unwrap_or(false)
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

    /// 启用或禁用 GNOME 桌面环境全局窗口与界面动画
    ///
    /// 当 enabled == false 时，GNOME Mutter 与 Shell 会彻底跳过所有窗口的缩放与淡入淡出动效，
    /// 实现 0ms 真正意义上的“瞬开瞬关”。
    pub fn set_gnome_animations_enabled(enabled: bool) -> bool {
        let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if !current_desktop.contains("GNOME") && !current_desktop.contains("ubuntu") {
            return false;
        }

        let val_str = if enabled { "true" } else { "false" };

        // 幂等性检查：若当前状态已与目标一致，跳过写入避免触发不必要的设置重载
        if let Ok(output) = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .output()
        {
            let current = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if current == val_str {
                return true;
            }
        }

        let status = Command::new("gsettings")
            .args(["set", "org.gnome.desktop.interface", "enable-animations", val_str])
            .status();

        match status {
            Ok(s) if s.success() => {
                info!("【桌面动效控制】已成功将 GNOME 桌面动画设置为: {}", val_str);
                true
            }
            Ok(s) => {
                tracing::warn!("设置 GNOME 桌面动画失败，退出码: {:?}", s.code());
                false
            }
            Err(e) => {
                tracing::warn!("调用 gsettings 控制动画失败: {:?}", e);
                false
            }
        }
    }

    /// 查询当前 GNOME 桌面环境是否开启了全局动画
    pub fn is_gnome_animations_enabled() -> bool {
        let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        if !current_desktop.contains("GNOME") && !current_desktop.contains("ubuntu") {
            return true;
        }

        if let Ok(output) = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .output()
        {
            let current = String::from_utf8_lossy(&output.stdout).trim().to_string();
            current == "true"
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_gnome_shortcut_does_not_panic() {
        let _ = DesktopShortcut::ensure_gnome_shortcut("F12");
    }

    #[test]
    fn test_is_gnome_animations_enabled_does_not_panic() {
        let _ = DesktopShortcut::is_gnome_animations_enabled();
    }
}
