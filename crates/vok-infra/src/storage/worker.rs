use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};
use rusqlite::{params, Connection};
use tracing::{debug, error, info};
use vok_core::model::{LauncherItem, StageItem};

use super::error::StorageError;
use super::schema::initialize_schema;

/// 投递给 StorageWorker 的写操作命令
pub(crate) enum StorageCommand {
    // 启动台写操作
    AddLauncherItem {
        item: LauncherItem,
        reply: Sender<Result<i64, StorageError>>,
    },
    DeleteLauncherItem {
        id: i64,
        reply: Sender<Result<(), StorageError>>,
    },
    ReorderLauncherItems {
        reorder: Vec<(i64, i32)>,
        reply: Sender<Result<(), StorageError>>,
    },

    // 中转站写操作
    AddStageItemsBatch {
        items: Vec<StageItem>,
        reply: Sender<Result<Vec<i64>, StorageError>>,
    },
    DeleteStageItem {
        id: i64,
        reply: Sender<Result<(), StorageError>>,
    },
    ReorderStageItems {
        reorder: Vec<(i64, i32)>,
        reply: Sender<Result<(), StorageError>>,
    },
    UpdateStageThumbnail {
        id: i64,
        thumbnail: PathBuf,
        reply: Sender<Result<(), StorageError>>,
    },
    SetStagePinned {
        id: i64,
        pinned: bool,
        reply: Sender<Result<(), StorageError>>,
    },
    ClearUnpinnedStageItems {
        reply: Sender<Result<usize, StorageError>>,
    },

    // 系统配置写操作
    SetSetting {
        key: String,
        value: String,
        reply: Sender<Result<(), StorageError>>,
    },

    // 终止写工作线程
    Shutdown,
}

/// 独占 SQLite 唯一写连接的后台工作者
pub struct StorageWorker {
    _handle: Option<JoinHandle<()>>,
}

impl StorageWorker {
    /// 启动独占写 Worker 线程并等待初始化就绪
    pub(crate) fn start(
        db_path: PathBuf,
        cmd_rx: Receiver<StorageCommand>,
    ) -> Result<Self, StorageError> {
        let (init_tx, init_rx) = bounded(1);
        let handle = thread::Builder::new()
            .name("vok-storage-worker".to_string())
            .spawn(move || {
                debug!("StorageWorker 线程启动，打开写连接: {:?}", db_path);
                let mut conn = match Connection::open(&db_path) {
                    Ok(c) => c,
                    Err(e) => {
                        error!("StorageWorker 无法打开数据库 {:?}: {}", db_path, e);
                        let _ = init_tx.send(Err(StorageError::Sqlite(e)));
                        return;
                    }
                };

                if let Err(e) = initialize_schema(&conn) {
                    error!("StorageWorker 初始化 schema 失败: {}", e);
                    let _ = init_tx.send(Err(StorageError::Sqlite(e)));
                    return;
                }

                let _ = init_tx.send(Ok(()));

                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        StorageCommand::AddLauncherItem { item, reply } => {
                            let res = Self::handle_add_launcher_item(&mut conn, item);
                            let _ = reply.send(res);
                        }
                        StorageCommand::DeleteLauncherItem { id, reply } => {
                            let res = Self::handle_delete_launcher_item(&mut conn, id);
                            let _ = reply.send(res);
                        }
                        StorageCommand::ReorderLauncherItems { reorder, reply } => {
                            let res = Self::handle_reorder_launcher_items(&mut conn, reorder);
                            let _ = reply.send(res);
                        }
                        StorageCommand::AddStageItemsBatch { items, reply } => {
                            let res = Self::handle_add_stage_items_batch(&mut conn, items);
                            let _ = reply.send(res);
                        }
                        StorageCommand::DeleteStageItem { id, reply } => {
                            let res = Self::handle_delete_stage_item(&mut conn, id);
                            let _ = reply.send(res);
                        }
                        StorageCommand::ReorderStageItems { reorder, reply } => {
                            let res = Self::handle_reorder_stage_items(&mut conn, reorder);
                            let _ = reply.send(res);
                        }
                        StorageCommand::UpdateStageThumbnail {
                            id,
                            thumbnail,
                            reply,
                        } => {
                            let res = Self::handle_update_stage_thumbnail(&mut conn, id, thumbnail);
                            let _ = reply.send(res);
                        }
                        StorageCommand::SetStagePinned { id, pinned, reply } => {
                            let res = Self::handle_set_stage_pinned(&mut conn, id, pinned);
                            let _ = reply.send(res);
                        }
                        StorageCommand::ClearUnpinnedStageItems { reply } => {
                            let res = Self::handle_clear_unpinned_stage_items(&mut conn);
                            let _ = reply.send(res);
                        }
                        StorageCommand::SetSetting { key, value, reply } => {
                            let res = Self::handle_set_setting(&mut conn, key, value);
                            let _ = reply.send(res);
                        }
                        StorageCommand::Shutdown => {
                            info!("StorageWorker 收到 Shutdown 信号，退出写事务循环");
                            break;
                        }
                    }
                }
            })
            .map_err(StorageError::Io)?;

        init_rx.recv().map_err(|_| StorageError::ChannelClosed)??;

        Ok(Self {
            _handle: Some(handle),
        })
    }

    fn handle_add_launcher_item(
        conn: &mut Connection,
        item: LauncherItem,
    ) -> Result<i64, StorageError> {
        conn.execute(
            "INSERT INTO launcher_items (title, target, icon_name, order_index, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                item.title,
                item.target,
                item.icon_name,
                item.order_index,
                item.created_at_ms,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    fn handle_delete_launcher_item(conn: &mut Connection, id: i64) -> Result<(), StorageError> {
        conn.execute("DELETE FROM launcher_items WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn handle_reorder_launcher_items(
        conn: &mut Connection,
        reorder: Vec<(i64, i32)>,
    ) -> Result<(), StorageError> {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE launcher_items SET order_index = ?1 WHERE id = ?2")?;
            for (id, order_index) in reorder {
                stmt.execute(params![order_index, id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn handle_add_stage_items_batch(
        conn: &mut Connection,
        items: Vec<StageItem>,
    ) -> Result<Vec<i64>, StorageError> {
        let tx = conn.transaction()?;
        let mut ids = Vec::with_capacity(items.len());
        {
            let mut stmt = tx.prepare(
                "INSERT INTO stage_items (
                    kind, uri, path, display_name, size_bytes, mime_type,
                    mtime_sec, thumbnail, pinned, order_index, created_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;

            for item in items {
                let thumb_str = item.thumbnail.map(|p| p.to_string_lossy().to_string());
                stmt.execute(params![
                    item.kind.as_str(),
                    item.uri,
                    item.path.to_string_lossy().to_string(),
                    item.display_name,
                    item.size_bytes as i64,
                    item.mime_type,
                    item.mtime_sec as i64,
                    thumb_str,
                    if item.pinned { 1 } else { 0 },
                    item.order_index,
                    item.created_at_ms,
                ])?;
                ids.push(tx.last_insert_rowid());
            }
        }
        tx.commit()?;
        Ok(ids)
    }

    fn handle_delete_stage_item(conn: &mut Connection, id: i64) -> Result<(), StorageError> {
        conn.execute("DELETE FROM stage_items WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn handle_reorder_stage_items(
        conn: &mut Connection,
        reorder: Vec<(i64, i32)>,
    ) -> Result<(), StorageError> {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE stage_items SET order_index = ?1 WHERE id = ?2")?;
            for (id, order_index) in reorder {
                stmt.execute(params![order_index, id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn handle_update_stage_thumbnail(
        conn: &mut Connection,
        id: i64,
        thumbnail: PathBuf,
    ) -> Result<(), StorageError> {
        conn.execute(
            "UPDATE stage_items SET thumbnail = ?1 WHERE id = ?2",
            params![thumbnail.to_string_lossy().to_string(), id],
        )?;
        Ok(())
    }

    fn handle_set_stage_pinned(
        conn: &mut Connection,
        id: i64,
        pinned: bool,
    ) -> Result<(), StorageError> {
        conn.execute(
            "UPDATE stage_items SET pinned = ?1 WHERE id = ?2",
            params![if pinned { 1 } else { 0 }, id],
        )?;
        Ok(())
    }

    fn handle_clear_unpinned_stage_items(conn: &mut Connection) -> Result<usize, StorageError> {
        let count = conn.execute("DELETE FROM stage_items WHERE pinned = 0", [])?;
        Ok(count)
    }

    fn handle_set_setting(
        conn: &mut Connection,
        key: String,
        value: String,
    ) -> Result<(), StorageError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
            params![key, value, now_ms],
        )?;
        Ok(())
    }
}
