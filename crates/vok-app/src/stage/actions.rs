use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use gtk4::gdk::Display;
use gtk4::prelude::*;
use tracing::{debug, info, warn};
use vok_core::model::StageItem;
use vok_infra::desktop::FileManager;
use vok_infra::storage::{StorageClient, StorageError};

/// 格式化文件尺寸为易读单位 (B / KB / MB / GB)
pub fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b < KB {
        format!("{} B", bytes)
    } else if b < MB {
        format!("{:.1} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.2} GB", b / GB)
    }
}

/// 根据文件后缀与属性推断 MIME 类型
pub fn detect_mime_type(path: &Path) -> String {
    if path.is_dir() {
        return "inode/directory".to_string();
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        // 图片
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" | "ico" | "tiff" => {
            format!("image/{}", if ext == "jpg" { "jpeg" } else { &ext })
        }
        // 视频
        "mp4" | "mkv" | "webm" | "avi" | "mov" | "flv" | "wmv" => {
            format!("video/{}", ext)
        }
        // 音频
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" => {
            format!("audio/{}", ext)
        }
        // 文本
        "txt" | "md" | "markdown" | "rst" | "json" | "xml" | "yaml" | "yml" | "toml" | "log" | "rs" | "py" | "c" | "cpp" | "h" | "sh" | "js" | "ts" | "css" | "html" => {
            "text/plain".to_string()
        }
        // 文档
        "pdf" => "application/pdf".to_string(),
        "doc" | "docx" | "odt" | "rtf" => "application/msword".to_string(),
        "xls" | "xlsx" | "ods" | "csv" => "application/vnd.ms-excel".to_string(),
        "ppt" | "pptx" | "odp" => "application/vnd.ms-powerpoint".to_string(),
        // 压缩包
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "zst" => {
            "application/zip".to_string()
        }
        // 默认常规二进制
        _ => "application/octet-stream".to_string(),
    }
}

/// 将外部拖入的文件路径批量解析并单事务写入存储 (遵循 R4 纯引用原则)
pub fn ingest_dropped_paths(
    storage: &StorageClient,
    paths: &[PathBuf],
    base_order: i32,
) -> Result<Vec<StageItem>, StorageError> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }

    info!("【中转站批量入库】正在解析并引用 {} 个外部文件...", paths.len());
    let mut items = Vec::with_capacity(paths.len());
    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    for (idx, path) in paths.iter().enumerate() {
        if !path.exists() {
            warn!("待入站路径不存在，跳过: {:?}", path);
            continue;
        }

        let display_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名文件".to_string());

        let metadata = std::fs::metadata(path);
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime_sec = metadata
            .as_ref()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mime_type = detect_mime_type(path);
        let canonical_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.clone());

        let item = StageItem::from_path_ref(
            0,
            &canonical_path,
            display_name,
            size_bytes,
            mime_type,
            mtime_sec,
            base_order + (idx as i32),
            now_ms,
        );

        items.push(item);
    }

    if items.is_empty() {
        return Ok(Vec::new());
    }

    // 单事务批量入库
    let ids = storage.add_stage_items_batch(items.clone())?;
    for (item, id) in items.iter_mut().zip(ids) {
        item.id = id;
    }

    debug!("【中转站批量入库成功】新增 {} 项", items.len());
    Ok(items)
}

/// 打开文件 (优先通过 gio，回退通过 xdg-open)
pub fn open_item(path: &Path) -> Result<(), String> {
    info!("【中转站打开文件】目标: {:?}", path);

    if !path.exists() {
        return Err("原文件已不存在或已被移动".to_string());
    }

    let uri = format!("file://{}", path.canonicalize().unwrap_or_else(|_| path.to_path_buf()).to_string_lossy());
    if let Some(app_info) = gtk4::gio::AppInfo::default_for_uri_scheme("file") {
        if app_info.launch_uris(&[&uri], None::<&gtk4::gio::AppLaunchContext>).is_ok() {
            return Ok(());
        }
    }

    // 回退到 xdg-open
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| format!("无法打开文件: {}", e))?;

    Ok(())
}

/// 在系统文件管理器中高亮定位文件
pub fn show_in_folder(path: &Path) -> Result<(), String> {
    FileManager::show_item_location(path).map_err(|e| e.to_string())
}

/// 复制源文件绝对路径至系统剪贴板
pub fn copy_path_to_clipboard(path: &Path) {
    if let Some(display) = Display::default() {
        display.clipboard().set_text(&path.to_string_lossy());
        info!("【剪贴板】已复制文件物理路径: {:?}", path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vok_core::model::StageItemKind;

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1024 * 1024 * 5), "5.0 MB");
        assert_eq!(format_file_size(1024 * 1024 * 1024 * 3), "3.00 GB");
    }

    #[test]
    fn test_detect_mime_type_and_kind() {
        let mime_png = detect_mime_type(Path::new("test.png"));
        assert_eq!(mime_png, "image/png");
        assert_eq!(StageItemKind::from_mime_type(&mime_png), StageItemKind::Image);

        let mime_txt = detect_mime_type(Path::new("note.md"));
        assert_eq!(mime_txt, "text/plain");
        assert_eq!(StageItemKind::from_mime_type(&mime_txt), StageItemKind::Text);

        let mime_pdf = detect_mime_type(Path::new("doc.pdf"));
        assert_eq!(mime_pdf, "application/pdf");
        assert_eq!(StageItemKind::from_mime_type(&mime_pdf), StageItemKind::File);
    }

    #[test]
    fn test_ingest_dropped_paths_batch() {
        let temp_dir = std::env::temp_dir().join(format!(
            "vok_stage_test_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let file1 = temp_dir.join("a.txt");
        let file2 = temp_dir.join("b.png");
        std::fs::write(&file1, "hello world").unwrap();
        std::fs::write(&file2, [0x89, 0x50, 0x4E, 0x47]).unwrap();

        let db_path = temp_dir.join("test.db");
        let storage = StorageClient::open(&db_path).unwrap();

        let ingested = ingest_dropped_paths(&storage, &[file1.clone(), file2.clone()], 0).unwrap();
        assert_eq!(ingested.len(), 2);
        assert!(ingested[0].id > 0);
        assert_eq!(ingested[0].display_name, "a.txt");
        assert_eq!(ingested[0].kind, StageItemKind::Text);
        assert_eq!(ingested[1].display_name, "b.png");
        assert_eq!(ingested[1].kind, StageItemKind::Image);

        // 验证 SQLite 中已持久化
        let list = storage.list_stage_items().unwrap();
        assert_eq!(list.len(), 2);

        // 验证 R4: 原文件依然完整无损
        assert!(file1.exists());
        assert!(file2.exists());

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
