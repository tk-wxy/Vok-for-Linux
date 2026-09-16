use std::path::Path;
use std::process::Command;

use tracing::{debug, warn};
use zbus::blocking::{Connection, Proxy};

use super::error::DesktopError;

pub struct FileManager;

impl FileManager {
    /// 在系统文件管理器中高亮定位并选中目标文件
    ///
    /// 优先调用 DBus `org.freedesktop.FileManager1.ShowItemLocations`；
    /// 若 DBus 服务不存在或调用失败，自动降级为使用 `xdg-open` 打开所在父目录。
    pub fn show_item_location(path: &Path) -> Result<(), DesktopError> {
        Self::show_items_locations(&[path])
    }

    /// 在系统文件管理器中批量高亮定位文件
    pub fn show_items_locations(paths: &[&Path]) -> Result<(), DesktopError> {
        if paths.is_empty() {
            return Ok(());
        }

        let mut uris = Vec::with_capacity(paths.len());
        for p in paths {
            let canonical = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            uris.push(format!("file://{}", canonical.to_string_lossy()));
        }

        // 1. 尝试 DBus FileManager1
        match Self::call_dbus_show_items(&uris) {
            Ok(()) => {
                debug!("通过 DBus FileManager1 成功触发高亮定位: {:?}", uris);
                return Ok(());
            }
            Err(e) => {
                warn!("DBus FileManager1 调用失败 ({})，尝试降级回退", e);
            }
        }

        // 2. 降级为 xdg-open 打开父目录
        if let Some(first_path) = paths.first() {
            Self::fallback_open_parent(first_path)
        } else {
            Ok(())
        }
    }

    fn call_dbus_show_items(uris: &[String]) -> Result<(), DesktopError> {
        let conn = Connection::session().map_err(|e| DesktopError::DBusError(e.to_string()))?;
        let proxy = Proxy::new(
            &conn,
            "org.freedesktop.FileManager1",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1",
        )
        .map_err(|e| DesktopError::DBusError(e.to_string()))?;

        let startup_id = "";
        let () = proxy
            .call("ShowItemLocations", &(uris, startup_id))
            .map_err(|e| DesktopError::DBusError(e.to_string()))?;

        Ok(())
    }

    fn fallback_open_parent(path: &Path) -> Result<(), DesktopError> {
        let parent = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };

        debug!("使用 xdg-open 打开父目录: {:?}", parent);
        Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(DesktopError::Io)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_manager_uris_generation() {
        let path = Path::new("/tmp/some_file.txt");
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let uri = format!("file://{}", canonical.to_string_lossy());
        assert!(uri.starts_with("file:///"));
    }
}
