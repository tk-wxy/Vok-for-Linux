use std::fmt;
use thiserror::Error;
use tracing::{info, warn};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
use x11rb::wrapper::ConnectionExt as _;

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

    /// 为指定标题的窗口附加 EWMH 跳过任务栏和跳过分页器属性
    pub fn apply_skip_taskbar_by_title(title: &str) -> bool {
        let (conn, screen_num) = match x11rb::connect(None) {
            Ok(c) => c,
            Err(e) => {
                warn!("连接 X11 失败，无法应用任务栏隐藏属性: {:?}", e);
                return false;
            }
        };
        let screen = &conn.setup().roots[screen_num];

        match Self::find_window_by_title(&conn, screen.root, title) {
            Ok(Some(win)) => {
                let res = Self::set_skip_taskbar(&conn, screen.root, win);
                if res.is_ok() {
                    info!("【任务栏彻底隐形】已成功对窗口 [{}] (XID: 0x{:x}) 设置 SKIP_TASKBAR 和 UTILITY", title, win);
                    true
                } else {
                    warn!("对窗口设置 SKIP_TASKBAR 失败: {:?}", res);
                    false
                }
            }
            Ok(None) => {
                warn!("未在 X11 窗口树中找到标题为 [{}] 的窗口", title);
                false
            }
            Err(e) => {
                warn!("遍历 X11 窗口树失败: {:?}", e);
                false
            }
        }
    }

    fn find_window_by_title(
        conn: &impl Connection,
        root: x11rb::protocol::xproto::Window,
        title: &str,
    ) -> Result<Option<x11rb::protocol::xproto::Window>, WindowError> {
        let tree = conn.query_tree(root)?.reply()?;
        let net_wm_name_atom = conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
        let wm_name_atom: u32 = AtomEnum::WM_NAME.into();

        for &child in &tree.children {
            // 先尝试 _NET_WM_NAME
            if let Ok(prop) = conn.get_property(
                false,
                child,
                net_wm_name_atom,
                AtomEnum::ANY,
                0,
                1024,
            ) {
                if let Ok(reply) = prop.reply() {
                    if !reply.value.is_empty() {
                        if let Ok(s) = String::from_utf8(reply.value) {
                            if s == title {
                                return Ok(Some(child));
                            }
                        }
                    }
                }
            }

            // 回退尝试 WM_NAME
            if let Ok(prop2) = conn.get_property(
                false,
                child,
                wm_name_atom,
                AtomEnum::ANY,
                0,
                1024,
            ) {
                if let Ok(reply) = prop2.reply() {
                    if !reply.value.is_empty() {
                        if let Ok(s) = String::from_utf8(reply.value) {
                            if s == title {
                                return Ok(Some(child));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn set_skip_taskbar(
        conn: &impl Connection,
        root: x11rb::protocol::xproto::Window,
        win: x11rb::protocol::xproto::Window,
    ) -> Result<(), WindowError> {
        use x11rb::protocol::xproto::{
            ClientMessageData, ClientMessageEvent, EventMask, PropMode, CLIENT_MESSAGE_EVENT,
        };

        let net_wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let skip_taskbar = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")?.reply()?.atom;
        let skip_pager = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_PAGER")?.reply()?.atom;
        let net_wm_window_type = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE")?.reply()?.atom;
        let window_type_utility = conn.intern_atom(false, b"_NET_WM_WINDOW_TYPE_UTILITY")?.reply()?.atom;

        // 1. 直接修改窗口属性
        conn.change_property32(
            PropMode::REPLACE,
            win,
            net_wm_state,
            AtomEnum::ATOM,
            &[skip_taskbar, skip_pager],
        )?;

        conn.change_property32(
            PropMode::REPLACE,
            win,
            net_wm_window_type,
            AtomEnum::ATOM,
            &[window_type_utility],
        )?;

        // 2. 发送 ClientMessage 通知窗口管理器 (Mutter / KWin)
        let event = ClientMessageEvent {
            response_type: CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window: win,
            type_: net_wm_state,
            data: ClientMessageData::from([
                1, // _NET_WM_STATE_ADD
                skip_taskbar,
                skip_pager,
                1,
                0,
            ]),
        };

        let event_mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        conn.send_event(false, root, event_mask, event)?;
        conn.flush()?;

        Ok(())
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
