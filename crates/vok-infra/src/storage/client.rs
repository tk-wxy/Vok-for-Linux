use std::path::{Path, PathBuf};

use crossbeam_channel::{bounded, Sender};
use rusqlite::{Connection, OpenFlags};
use tracing::debug;
use vok_core::model::{LauncherItem, StageItem, StageItemKind};

use super::error::StorageError;
use super::worker::{StorageCommand, StorageWorker};
use super::xdg;

/// 线程安全的持久化存储客户端句柄 (可安全 Clone 并跨线程传递)
#[derive(Clone)]
pub struct StorageClient {
    db_path: PathBuf,
    cmd_tx: Sender<StorageCommand>,
}

impl StorageClient {
    /// 打开指定路径的数据库并启动独占写 Worker
    pub fn open(db_path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let db_path = db_path.into();
        // 如果父目录不存在则先行创建
        if let Some(parent) = db_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let (cmd_tx, cmd_rx) = bounded(256);
        StorageWorker::start(db_path.clone(), cmd_rx)?;

        Ok(Self { db_path, cmd_tx })
    }

    /// 打开 XDG 规范的默认主数据库 ($XDG_DATA_HOME/vok/vok.db)
    pub fn open_default() -> Result<Self, StorageError> {
        xdg::ensure_storage_dirs()?;
        Self::open(xdg::get_vok_db_path())
    }

    /// 获取当前数据库物理文件绝对路径
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// 打开一个新的只读连接（得益于 WAL 模式，可并发任意多个只读连接，与写事务零互斥）
    pub fn open_read_connection(&self) -> Result<Connection, StorageError> {
        let conn = Connection::open_with_flags(
            &self.db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;
            ",
        )?;
        Ok(conn)
    }

    // ==================== 启动台 (Launcher) 接口 ====================

    /// 查询全部启动项（按 order_index 升序排列）
    pub fn list_launcher_items(&self) -> Result<Vec<LauncherItem>, StorageError> {
        let conn = self.open_read_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, target, icon_name, order_index, created_at_ms
             FROM launcher_items ORDER BY order_index ASC, id ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(LauncherItem::new(
                row.get(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?;

        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    /// 新增启动项，返回自增 ID
    pub fn add_launcher_item(&self, item: LauncherItem) -> Result<i64, StorageError> {
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::AddLauncherItem { item, reply })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    /// 根据 ID 删除指定启动项
    pub fn delete_launcher_item(&self, id: i64) -> Result<(), StorageError> {
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::DeleteLauncherItem { id, reply })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    /// 批量更新启动项的排序次序
    pub fn reorder_launcher_items(&self, reorder: Vec<(i64, i32)>) -> Result<(), StorageError> {
        if reorder.is_empty() {
            return Ok(());
        }
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::ReorderLauncherItems { reorder, reply })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    // ==================== 中转站 (Stage) 接口 ====================

    /// 查询全部中转项（优先置顶 pinned 降序，再按 order_index 升序）
    pub fn list_stage_items(&self) -> Result<Vec<StageItem>, StorageError> {
        let conn = self.open_read_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, uri, path, display_name, size_bytes, mime_type,
                    mtime_sec, thumbnail, pinned, order_index, created_at_ms
             FROM stage_items ORDER BY pinned DESC, order_index ASC, id DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let kind_str: String = row.get(1)?;
            let kind = StageItemKind::from_str_kind(&kind_str).unwrap_or(StageItemKind::File);
            let path_str: String = row.get(3)?;
            let thumb_str: Option<String> = row.get(8)?;
            let pinned_int: i32 = row.get(9)?;

            Ok(StageItem::new(
                row.get(0)?,
                kind,
                row.get::<_, String>(2)?,
                PathBuf::from(path_str),
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)? as u64,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)? as u64,
                thumb_str.map(PathBuf::from),
                pinned_int != 0,
                row.get(10)?,
                row.get(11)?,
            ))
        })?;

        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    /// 单次事务批量添加中转项（入站拖入秒级持久化）
    pub fn add_stage_items_batch(&self, items: Vec<StageItem>) -> Result<Vec<i64>, StorageError> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::AddStageItemsBatch { items, reply })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    /// 删除指定中转项
    pub fn delete_stage_item(&self, id: i64) -> Result<(), StorageError> {
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::DeleteStageItem { id, reply })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    /// 批量更新中转项排序次序
    pub fn reorder_stage_items(&self, reorder: Vec<(i64, i32)>) -> Result<(), StorageError> {
        if reorder.is_empty() {
            return Ok(());
        }
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::ReorderStageItems { reorder, reply })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    /// 更新中转项的缩略图路径
    pub fn update_stage_thumbnail(&self, id: i64, thumbnail: PathBuf) -> Result<(), StorageError> {
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::UpdateStageThumbnail {
                id,
                thumbnail,
                reply,
            })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    /// 设置中转项置顶状态
    pub fn set_stage_pinned(&self, id: i64, pinned: bool) -> Result<(), StorageError> {
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::SetStagePinned { id, pinned, reply })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    /// 清理所有未置顶锁定的中转项
    pub fn clear_unpinned_stage_items(&self) -> Result<usize, StorageError> {
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::ClearUnpinnedStageItems { reply })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    // ==================== 通用配置 (Settings) 接口 ====================

    /// 查询指定 key 的配置项 (利用 WAL 无锁只读并发)
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        let conn = self.open_read_connection()?;
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let mut rows = stmt.query(rusqlite::params![key])?;
        if let Some(row) = rows.next()? {
            let val: String = row.get(0)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    /// 设置配置项 (由 StorageWorker 串行持久化落盘)
    pub fn set_setting(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), StorageError> {
        let (reply, rx) = bounded(1);
        self.cmd_tx
            .send(StorageCommand::SetSetting {
                key: key.into(),
                value: value.into(),
                reply,
            })
            .map_err(|_| StorageError::ChannelClosed)?;
        rx.recv().map_err(|_| StorageError::ChannelClosed)?
    }

    /// 主动终止写 Worker 线程
    pub fn shutdown(&self) {
        debug!("向 StorageWorker 发送 Shutdown 信号");
        let _ = self.cmd_tx.send(StorageCommand::Shutdown);
    }
}
