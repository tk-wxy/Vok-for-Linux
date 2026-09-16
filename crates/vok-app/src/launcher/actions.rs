use std::path::Path;
use std::process::Command;

use gtk4::gio;
use gtk4::prelude::*;
use tracing::{debug, info, warn};
use vok_core::model::LauncherItem;
use vok_infra::desktop::{DesktopEntry, FileManager};
use vok_infra::storage::StorageClient;

/// 启动目标应用或打开文件
pub fn launch_target(target: &str) -> Result<(), String> {
    info!("正在执行启动目标: {}", target);

    // 1. 如果是 .desktop 文件
    if target.ends_with(".desktop") {
        // 优先使用 GIO DesktopAppInfo
        if let Some(app_info) = gio::DesktopAppInfo::from_filename(target) {
            if let Err(e) = app_info.launch(&[], None::<&gio::AppLaunchContext>) {
                warn!("GIO DesktopAppInfo 启动失败 ({})，尝试读取 Exec 执行", e);
            } else {
                debug!("GIO DesktopAppInfo 成功拉起应用: {}", target);
                return Ok(());
            }
        }

        // 回退尝试手动解析 Exec
        if let Ok(entry) = DesktopEntry::from_file(Path::new(target)) {
            if !entry.exec.is_empty() {
                return execute_shell_command(&entry.exec);
            }
        }
    }

    // 2. 普通可执行命令或绝对路径
    execute_shell_command(target)
}

fn execute_shell_command(cmd_str: &str) -> Result<(), String> {
    debug!("通过系统 Shell 派发命令: {}", cmd_str);
    Command::new("sh")
        .arg("-c")
        .arg(format!("{} &", cmd_str))
        .spawn()
        .map_err(|e| format!("命令派发失败: {}", e))?;
    Ok(())
}

/// 在文件管理器中高亮定位目标
pub fn show_in_folder(target: &str) -> Result<(), String> {
    FileManager::show_item_location(Path::new(target)).map_err(|e| e.to_string())
}

/// 当数据库为空时，自动扫描系统并注入默认启动项 (避免冷启动空屏)
pub fn seed_default_launcher_items(storage: &StorageClient) {
    if let Ok(existing) = storage.list_launcher_items() {
        if !existing.is_empty() {
            return;
        }
    }

    info!("启动台数据为空，开始初始化默认系统应用...");

    let candidates = [
        // 浏览器类
        &["firefox.desktop", "google-chrome.desktop", "chromium.desktop"][..],
        // 文件管理器类
        &["org.gnome.Nautilus.desktop", "nautilus.desktop", "dolphin.desktop", "thunar.desktop"][..],
        // 终端类
        &["org.gnome.Terminal.desktop", "alacritty.desktop", "kitty.desktop", "org.gnome.Console.desktop"][..],
        // 编辑器类
        &["code.desktop", "org.gnome.TextEditor.desktop", "gedit.desktop"][..],
    ];

    let mut seeded = 0;
    let base_dirs = [
        Path::new("/usr/share/applications"),
        Path::new("/usr/local/share/applications"),
    ];

    for group in &candidates {
        for name in *group {
            let mut found = None;
            for dir in &base_dirs {
                let p = dir.join(name);
                if p.exists() {
                    found = Some(p);
                    break;
                }
            }

            if let Some(path) = found {
                if let Ok(entry) = DesktopEntry::from_file(&path) {
                    let item = LauncherItem::new(
                        0,
                        entry.name,
                        path.to_string_lossy().to_string(),
                        entry.icon,
                        seeded,
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64,
                    );
                    if let Ok(id) = storage.add_launcher_item(item) {
                        debug!("已注入初始启动项: {} (id: {})", name, id);
                        seeded += 1;
                        break;
                    }
                }
            }
        }
    }

    // 若系统未安装标准桌面应用，生成 4 个演示项保底
    if seeded == 0 {
        let fallbacks = [
            ("文件管理器", "xdg-open .", "system-file-manager"),
            ("终端控制台", "x-terminal-emulator", "utilities-terminal"),
            ("系统设置", "gnome-control-center", "preferences-system"),
            ("文本编辑", "gedit", "accessories-text-editor"),
        ];

        for (idx, (title, target, icon)) in fallbacks.iter().enumerate() {
            let item = LauncherItem::new(
                0,
                *title,
                *target,
                *icon,
                idx as i32,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            );
            let _ = storage.add_launcher_item(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seed_default_launcher_items() {
        let db_path = std::env::temp_dir().join(format!(
            "vok_launcher_seed_test_{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let storage = StorageClient::open(&db_path).expect("打开测试数据库失败");

        // 初始为空
        let items_before = storage.list_launcher_items().expect("查询初始列表失败");
        assert!(items_before.is_empty());

        // 执行注入
        seed_default_launcher_items(&storage);

        // 验证已注入（来自真实系统应用或 4 个保底项）
        let items_after = storage.list_launcher_items().expect("查询注入后列表失败");
        assert!(!items_after.is_empty());

        // 重复调用不应重复注入
        seed_default_launcher_items(&storage);
        let items_second = storage.list_launcher_items().expect("查询二次注入后列表失败");
        assert_eq!(items_after.len(), items_second.len());

        let _ = std::fs::remove_file(db_path);
    }
}

