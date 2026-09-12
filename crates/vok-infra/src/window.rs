use std::fmt;
use thiserror::Error;
use tracing::{info, warn};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

#[derive(Error, Debug)]
pub enum WindowError {
    #[error("X11 回复解析错误: {0}")]
    Reply(#[from] x11rb::errors::ReplyError),
    #[error("X11 连接通信错误: {0}")]
    Connection(#[from] x11rb::errors::ConnectionError),
    #[error("X11 建立连接失败: {0}")]
    Connect(#[from] x11rb::errors::ConnectError),
    #[error("未找到可用的工作区配置")]
    NoWorkAreaFound,
}

/// 屏幕工作区几何区域（避让系统任务栏/Dock 后的可用区域）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkAreaGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl fmt::Display for WorkAreaGeometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{}+{}+{}",
            self.width, self.height, self.x, self.y
        )
    }
}

pub struct WindowAdapter;

impl WindowAdapter {
    /// 查询当前桌面环境可用工作区几何尺寸（避让系统 Dock/Panel）
    pub fn get_work_area() -> WorkAreaGeometry {
        // 尝试从 X11 EWMH _NET_WORKAREA 查询
        match Self::query_x11_workarea() {
            Ok(geom) => {
                info!("成功获取 X11 工作区尺寸 (避让任务栏): {}", geom);
                geom
            }
            Err(e) => {
                warn!("查询 X11 工作区属性失败 ({:?})，回退默认全屏 1920x1080", e);
                WorkAreaGeometry {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                }
            }
        }
    }

    fn query_x11_workarea() -> Result<WorkAreaGeometry, WindowError> {
        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];

        let net_workarea_atom = conn
            .intern_atom(false, b"_NET_WORKAREA")?
            .reply()?
            .atom;

        let reply = conn
            .get_property(
                false,
                screen.root,
                net_workarea_atom,
                AtomEnum::CARDINAL,
                0,
                4, // 读取前 4 个 32 位值 (x, y, width, height)
            )?
            .reply()?;

        if let Some(mut iter) = reply.value32() {
            let x = iter.next().unwrap_or(0) as i32;
            let y = iter.next().unwrap_or(0) as i32;
            let width = iter.next().unwrap_or(screen.width_in_pixels as u32);
            let height = iter.next().unwrap_or(screen.height_in_pixels as u32);

            return Ok(WorkAreaGeometry {
                x,
                y,
                width,
                height,
            });
        }

        // 若无 _NET_WORKAREA 属性，默认使用屏幕全屏尺寸
        Ok(WorkAreaGeometry {
            x: 0,
            y: 0,
            width: screen.width_in_pixels as u32,
            height: screen.height_in_pixels as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workarea_display() {
        let geom = WorkAreaGeometry {
            x: 48,
            y: 32,
            width: 1872,
            height: 1048,
        };
        assert_eq!(format!("{geom}"), "1872x1048+48+32");
    }

    #[test]
    fn test_query_workarea_does_not_panic() {
        let geom = WindowAdapter::get_work_area();
        assert!(geom.width > 0);
        assert!(geom.height > 0);
    }
}
