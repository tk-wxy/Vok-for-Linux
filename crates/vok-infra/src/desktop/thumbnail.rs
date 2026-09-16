use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::SystemTime;

use crossbeam_channel::{bounded, Receiver, Sender};
use image::ImageFormat;
use tracing::{debug, error, info, warn};

use super::error::DesktopError;
use crate::storage::xdg;

/// 最大缩略图尺寸限制 (160x160 保持宽高比)
pub const MAX_THUMB_SIZE: u32 = 160;

/// 磁盘缓存配额软上限 (50MB)
pub const MAX_DISK_QUOTA_BYTES: u64 = 50 * 1024 * 1024;

/// 触发清理后的目标配额 (40MB)
pub const TARGET_DISK_QUOTA_BYTES: u64 = 40 * 1024 * 1024;

/// 异步缩略图生成任务
pub(crate) struct ThumbnailTask {
    pub item_id: i64,
    pub source_path: PathBuf,
    pub mtime_sec: u64,
    pub on_complete: Box<dyn FnOnce(i64, Option<PathBuf>) + Send + 'static>,
}

pub(crate) enum ThumbnailCommand {
    Generate(ThumbnailTask),
    Shutdown,
}

/// 后台异步缩略图生成器
pub struct ThumbnailWorker {
    task_tx: Sender<ThumbnailCommand>,
    _handle: Option<JoinHandle<()>>,
}

impl ThumbnailWorker {
    /// 启动缩略图后台工作线程
    pub fn start() -> Result<Self, DesktopError> {
        let (task_tx, task_rx): (Sender<ThumbnailCommand>, Receiver<ThumbnailCommand>) =
            bounded(128);

        let handle = thread::Builder::new()
            .name("vok-thumbnail-worker".to_string())
            .spawn(move || {
                debug!("ThumbnailWorker 线程启动，监听缩略图生成任务");
                while let Ok(cmd) = task_rx.recv() {
                    match cmd {
                        ThumbnailCommand::Generate(task) => {
                            let thumb_path = Self::generate_sync(&task.source_path, task.mtime_sec);
                            (task.on_complete)(task.item_id, thumb_path);
                        }
                        ThumbnailCommand::Shutdown => {
                            info!("ThumbnailWorker 收到 Shutdown 信号，退出循环");
                            break;
                        }
                    }
                }
            })
            .map_err(DesktopError::Io)?;

        Ok(Self {
            task_tx,
            _handle: Some(handle),
        })
    }

    /// 投递异步缩略图生成请求 (若已命中缓存则瞬时返回)
    pub fn request_thumbnail<F>(&self, item_id: i64, source_path: PathBuf, mtime_sec: u64, on_complete: F)
    where
        F: FnOnce(i64, Option<PathBuf>) + Send + 'static,
    {
        // 先检查本地是否已有缓存命中
        if let Some(cached) = Self::get_cached_path(&source_path, mtime_sec) {
            on_complete(item_id, Some(cached));
            return;
        }

        let task = ThumbnailTask {
            item_id,
            source_path,
            mtime_sec,
            on_complete: Box::new(on_complete),
        };

        if let Err(e) = self.task_tx.send(ThumbnailCommand::Generate(task)) {
            warn!("向 ThumbnailWorker 投递任务失败: {}", e);
        }
    }

    /// 同步生成缩略图并写入缓存 (命中缓存则直接返回现有路径)
    pub fn generate_sync(source_path: &Path, mtime_sec: u64) -> Option<PathBuf> {
        if !source_path.exists() {
            return None;
        }

        // 仅处理常见图片格式
        if !Self::is_supported_image(source_path) {
            return None;
        }

        let thumbs_dir = xdg::get_vok_thumbs_dir();
        if !thumbs_dir.exists() {
            let _ = fs::create_dir_all(&thumbs_dir);
        }

        let target_filename = Self::compute_thumb_filename(source_path, mtime_sec);
        let target_path = thumbs_dir.join(target_filename);

        if target_path.exists() {
            return Some(target_path);
        }

        // 解码并缩放图片
        match image::open(source_path) {
            Ok(img) => {
                let thumb = img.thumbnail(MAX_THUMB_SIZE, MAX_THUMB_SIZE);
                if let Err(e) = thumb.save_with_format(&target_path, ImageFormat::Png) {
                    error!("保存缩略图失败 {:?}: {}", target_path, e);
                    return None;
                }
                debug!("缩略图生成成功: {:?}", target_path);

                // 检查并维护磁盘配额
                Self::enforce_disk_quota(&thumbs_dir);

                Some(target_path)
            }
            Err(e) => {
                debug!("文件非图像或解码失败 {:?}: {}", source_path, e);
                None
            }
        }
    }

    /// 检查并获取已存在的有效缓存缩略图
    pub fn get_cached_path(source_path: &Path, mtime_sec: u64) -> Option<PathBuf> {
        let target_filename = Self::compute_thumb_filename(source_path, mtime_sec);
        let target_path = xdg::get_vok_thumbs_dir().join(target_filename);
        if target_path.exists() {
            Some(target_path)
        } else {
            None
        }
    }

    /// 计算缩略图确定的缓存文件名 (由物理路径哈希与修改时间组合确定)
    pub fn compute_thumb_filename(source_path: &Path, mtime_sec: u64) -> String {
        let mut hasher = DefaultHasher::new();
        source_path.hash(&mut hasher);
        let hash = hasher.finish();
        format!("{:016x}_{}.png", hash, mtime_sec)
    }

    /// 检查扩展名是否属于受支持的图像文件
    fn is_supported_image(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_lowercase();
        matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
        )
    }

    /// 检查并维护磁盘缓存配额 (超 50MB 时 LRU 清理至 40MB)
    pub fn enforce_disk_quota(thumbs_dir: &Path) {
        let mut files = Vec::new();
        let mut total_bytes = 0u64;

        if let Ok(entries) = fs::read_dir(thumbs_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        let len = meta.len();
                        total_bytes += len;
                        let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                        files.push((entry.path(), len, mtime));
                    }
                }
            }
        }

        if total_bytes > MAX_DISK_QUOTA_BYTES {
            info!(
                "缩略图缓存体积超过 50MB 阈值 (当前: {} 字节)，启动 LRU 清理",
                total_bytes
            );
            // 按修改时间升序排列 (最旧的文件排在前面)
            files.sort_by_key(|(_, _, mtime)| *mtime);

            for (path, len, _) in files {
                if total_bytes <= TARGET_DISK_QUOTA_BYTES {
                    break;
                }
                if fs::remove_file(&path).is_ok() {
                    total_bytes = total_bytes.saturating_sub(len);
                }
            }
            debug!("LRU 清理完成，剩余缩略图缓存体积: {} 字节", total_bytes);
        }
    }

    /// 退出工作线程
    pub fn shutdown(&self) {
        let _ = self.task_tx.send(ThumbnailCommand::Shutdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn test_thumbnail_generation_and_caching() {
        let temp_dir = std::env::temp_dir();
        let test_img_path = temp_dir.join("vok_test_source_image.png");

        // 创建一个 400x200 的测试 RGB 图片
        let mut img = RgbImage::new(400, 200);
        for pixel in img.pixels_mut() {
            *pixel = Rgb([100, 150, 200]);
        }
        img.save(&test_img_path).expect("save test image");

        // 生成缩略图
        let thumb_path =
            ThumbnailWorker::generate_sync(&test_img_path, 1726000000).expect("generate thumb");

        assert!(thumb_path.exists());
        // 校验缩略图尺寸是否在 160px 约束内
        let loaded_thumb = image::open(&thumb_path).expect("open thumb");
        assert!(loaded_thumb.width() <= MAX_THUMB_SIZE);
        assert!(loaded_thumb.height() <= MAX_THUMB_SIZE);
        // 400x200 保持纵横比缩放到 160 宽，则高应为 80
        assert_eq!(loaded_thumb.width(), 160);
        assert_eq!(loaded_thumb.height(), 80);

        // 再次获取应直接命中缓存
        let cached = ThumbnailWorker::get_cached_path(&test_img_path, 1726000000);
        assert_eq!(cached, Some(thumb_path.clone()));

        // 清理测试图片与生成的缩略图
        let _ = fs::remove_file(test_img_path);
        let _ = fs::remove_file(thumb_path);
    }

    #[test]
    fn test_non_image_returns_none() {
        let temp_dir = std::env::temp_dir();
        let txt_path = temp_dir.join("vok_test_not_an_image.txt");
        fs::write(&txt_path, b"not an image content").unwrap();

        let res = ThumbnailWorker::generate_sync(&txt_path, 1726000000);
        assert_eq!(res, None);

        let _ = fs::remove_file(txt_path);
    }
}
