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

    /// 读取当前桌面活动窗口的 XID (EWMH _NET_ACTIVE_WINDOW)
    pub fn get_active_window_id() -> Option<u32> {
        let (conn, screen_num) = x11rb::connect(None).ok()?;
        let screen = &conn.setup().roots[screen_num];

        let net_active_window_atom = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .ok()?
            .reply()
            .ok()?
            .atom;

        let reply = conn
            .get_property(
                false,
                screen.root,
                net_active_window_atom,
                AtomEnum::WINDOW,
                0,
                1,
            )
            .ok()?
            .reply()
            .ok()?;

        if let Some(mut iter) = reply.value32() {
            let win = iter.next()?;
            if win != 0 {
                return Some(win);
            }
        }
        None
    }

    /// 获取指定窗口的标题 (用于日志诊断与识别自身)
    pub fn get_window_title(xid: u32) -> Option<String> {
        let (conn, _) = x11rb::connect(None).ok()?;
        let net_wm_name_atom = conn.intern_atom(false, b"_NET_WM_NAME").ok()?.reply().ok()?.atom;
        let wm_name_atom: u32 = AtomEnum::WM_NAME.into();

        if let Ok(prop) = conn.get_property(false, xid, net_wm_name_atom, AtomEnum::ANY, 0, 1024) {
            if let Ok(reply) = prop.reply() {
                if !reply.value.is_empty() {
                    if let Ok(s) = String::from_utf8(reply.value) {
                        return Some(s);
                    }
                }
            }
        }
        if let Ok(prop) = conn.get_property(false, xid, wm_name_atom, AtomEnum::ANY, 0, 1024) {
            if let Ok(reply) = prop.reply() {
                if !reply.value.is_empty() {
                    if let Ok(s) = String::from_utf8(reply.value) {
                        return Some(s);
                    }
                }
            }
        }
        None
    }

    /// 向窗口管理器 (Mutter / KWin) 申请恢复指定窗口的活动前台焦点
    /// 发送 _NET_ACTIVE_WINDOW ClientMessage (source indication = 2 绕过防焦点窃取限制)
    pub fn restore_active_window(xid: u32) -> Result<(), WindowError> {
        if xid == 0 {
            return Ok(());
        }

        let (conn, screen_num) = x11rb::connect(None)?;
        let screen = &conn.setup().roots[screen_num];

        let net_active_window = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
            .reply()?
            .atom;

        use x11rb::protocol::xproto::{
            ClientMessageData, ClientMessageEvent, EventMask, CLIENT_MESSAGE_EVENT,
        };

        let event_mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        let event_active = ClientMessageEvent {
            response_type: CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window: xid,
            type_: net_active_window,
            data: ClientMessageData::from([
                2, // 2: Pager / Taskbar / System Panel (高优先级切换)
                0, // CurrentTime
                0,
                0,
                0,
            ]),
        };

        conn.send_event(false, screen.root, event_mask, event_active)?;
        conn.flush()?;

        info!("【焦点精准返还】已向窗口管理器申请激活原前台窗口 [0x{:x}]", xid);
        Ok(())
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
                    info!("【任务栏隐形与工作区贴合】已成功对窗口 [{}] (XID: 0x{:x}) 设置 SKIP_TASKBAR, ABOVE 和 MAXIMIZED", title, win);
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
        let max_horz = conn
            .intern_atom(false, b"_NET_WM_STATE_MAXIMIZED_HORZ")?
            .reply()?
            .atom;
        let max_vert = conn
            .intern_atom(false, b"_NET_WM_STATE_MAXIMIZED_VERT")?
            .reply()?
            .atom;
        let skip_taskbar = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")?.reply()?.atom;
        let skip_pager = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_PAGER")?.reply()?.atom;
        let state_above = conn.intern_atom(false, b"_NET_WM_STATE_ABOVE")?.reply()?.atom;
        let stays_on_top = conn.intern_atom(false, b"_NET_WM_STATE_STAYS_ON_TOP")?.reply()?.atom;
        let net_wm_window_type = conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
            .reply()?
            .atom;
        let window_type_normal = conn
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE_NORMAL")?
            .reply()?
            .atom;
        let net_active_window = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
            .reply()?
            .atom;

        // 1. 设置窗口属性 (权威固化最大化、跳过任务栏、分页器、强制置顶属性)
        tracing::debug!(
            "【X11 Atoms】max_h={}, max_v={}, skip_t={}, skip_p={}, above={}, on_top={}",
            max_horz, max_vert, skip_taskbar, skip_pager, state_above, stays_on_top
        );
        conn.change_property32(
            PropMode::REPLACE,
            win,
            net_wm_state,
            AtomEnum::ATOM,
            &[
                max_horz,
                max_vert,
                skip_taskbar,
                skip_pager,
                state_above,
                stays_on_top,
            ],
        )?;

        conn.change_property32(
            PropMode::REPLACE,
            win,
            net_wm_window_type,
            AtomEnum::ATOM,
            &[window_type_normal],
        )?;

        let event_mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;

        // 2. 发送 ClientMessage 通知窗口管理器附加最大化状态
        let event_max = ClientMessageEvent {
            response_type: CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window: win,
            type_: net_wm_state,
            data: ClientMessageData::from([
                1, // _NET_WM_STATE_ADD
                max_horz,
                max_vert,
                1,
                0,
            ]),
        };
        conn.send_event(false, root, event_mask, event_max)?;

        // 3. 发送 ClientMessage 通知窗口管理器 (Mutter / KWin) 附加置顶状态
        let event_above = ClientMessageEvent {
            response_type: CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window: win,
            type_: net_wm_state,
            data: ClientMessageData::from([
                1, // _NET_WM_STATE_ADD
                state_above,
                stays_on_top,
                1,
                0,
            ]),
        };
        conn.send_event(false, root, event_mask, event_above)?;

        // 3. 发送 ClientMessage 通知窗口管理器附加跳过任务栏
        let event_skip = ClientMessageEvent {
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
        conn.send_event(false, root, event_mask, event_skip)?;

        // 5. 发送 _NET_ACTIVE_WINDOW 强制激活窗口并提升至最前台 (source indication = 2 绕过防焦点窃取)
        let event_active = ClientMessageEvent {
            response_type: CLIENT_MESSAGE_EVENT,
            format: 32,
            sequence: 0,
            window: win,
            type_: net_active_window,
            data: ClientMessageData::from([
                2, // 2: Pager / Taskbar / System Panel (优先级高于普通应用)
                0, // CurrentTime
                0,
                0,
                0,
            ]),
        };
        conn.send_event(false, root, event_mask, event_active)?;

        // 6. 硬件级提升窗口层级 (相当于 XRaiseWindow / StackMode::ABOVE)
        // 注意：严禁在运行中对已最大化窗口强制传入 x, y, width, height，避免与合成器布局死锁导致缩放抽搐
        use x11rb::protocol::xproto::{ConfigureWindowAux, StackMode};
        let aux = ConfigureWindowAux::new().stack_mode(StackMode::ABOVE);
        let _ = conn.configure_window(win, &aux);

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

    #[test]
    fn test_active_window_query_and_restore_does_not_panic() {
        let active = WindowAdapter::get_active_window_id();
        if let Some(xid) = active {
            let _ = WindowAdapter::get_window_title(xid);
            let res = WindowAdapter::restore_active_window(xid);
            assert!(res.is_ok());
        }
        // xid == 0 应当作为安全 no-op 处理成功
        assert!(WindowAdapter::restore_active_window(0).is_ok());
    }
}
