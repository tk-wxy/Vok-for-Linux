use thiserror::Error;

#[derive(Debug, Error)]
pub enum DesktopError {
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error(".desktop 文件语法解析错误: {0}")]
    ParseError(String),

    #[error("DBus 协议通信错误: {0}")]
    DBusError(String),

    #[error("图像解码/生成错误: {0}")]
    ImageError(String),

    #[error("ThumbnailWorker 通道已断开")]
    ChannelClosed,
}
