use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite 数据库错误: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("StorageWorker 通道已关闭或发送超时")]
    ChannelClosed,

    #[error("目标条目不存在: id {0}")]
    NotFound(i64),
}
