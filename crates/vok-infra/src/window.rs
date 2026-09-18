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

    fn with_cached_conn<R, F: FnOnce(&x11rb::rust_connection::RustConnection, usize) -> R>(f: F) -> Option<R> {
        use std::cell::RefCell;
        use x11rb::rust_connection::RustConnection;
        thread_local! {
            static CONN: RefCell<Option<(RustConnection, usize)>> = const { RefCell::new(None) };
        }
        CONN.with(|cell| {
            let mut opt = cell.borrow_mut();
            if opt.is_none() {
                *opt = x11rb::connect(None).ok();
            }
            let (conn, screen_num) = opt.as_ref()?;
            Some(f(conn, *screen_num))
        })
    }

    /// 查询鼠标光标是否在指定工作区外部被按下 (如点击屏幕边缘的 Ubuntu Dock 或 GNOME 顶栏)
    pub fn is_pointer_clicking_outside(workarea: WorkAreaGeometry) -> bool {
        Self::with_cached_conn(|conn, screen_num| {
            let screen = &conn.setup().roots[screen_num];
            if let Ok(cookie) = conn.query_pointer(screen.root) {
                if let Ok(reply) = cookie.reply() {
                    let x = reply.root_x as i32;
                    let y = reply.root_y as i32;
                    let mask: u16 = reply.mask.into();
                    let button_pressed = (mask & (
                        u16::from(x11rb::protocol::xproto::KeyButMask::BUTTON1)
                        | u16::from(x11rb::protocol::xproto::KeyButMask::BUTTON2)
                        | u16::from(x11rb::protocol::xproto::KeyButMask::BUTTON3)
                    )) != 0;

                    let is_outside = x < workarea.x
                        || y < workarea.y
                        || x >= (workarea.x + workarea.width as i32)
                        || y >= (workarea.y + workarea.height as i32);

                    return button_pressed && is_outside;
                }
            }
            false
        }).unwrap_or(false)
    }

    /// 查询当前全局指针在根窗口上的绝对物理坐标 (root_x, root_y)
    pub fn get_pointer_position() -> Option<(i32, i32)> {
        Self::with_cached_conn(|conn, screen_num| {
            let screen = &conn.setup().roots[screen_num];
            if let Ok(cookie) = conn.query_pointer(screen.root) {
                if let Ok(reply) = cookie.reply() {
                    return Some((reply.root_x as i32, reply.root_y as i32));
                }
            }
            None
        }).flatten()
    }

    /// 综合判定当前物理光标是否位于 Vok 窗口外部
    ///
    /// 返回 Option<(is_outside, x, y, child_xid)>
    /// - 若几何坐标超出工作区：is_outside = true
    /// - 若当前光标下的顶级子窗口不是 Vok (vok_xid)：is_outside = true (彻底攻克 Wayland 任务栏坐标钳位与合成器吸收死穴)
    pub fn check_pointer_against_vok(wa: WorkAreaGeometry, vok_xid: u32) -> Option<(bool, i32, i32, u32)> {
        Self::with_cached_conn(|conn, screen_num| {
            let screen = &conn.setup().roots[screen_num];
            if let Ok(cookie) = conn.query_pointer(screen.root) {
                if let Ok(reply) = cookie.reply() {
                    let x = reply.root_x as i32;
                    let y = reply.root_y as i32;
                    let child = reply.child;
                    let geom_outside = x < wa.x || y < wa.y || x >= (wa.x + wa.width as i32) || y >= (wa.y + wa.height as i32);
                    let window_outside = if vok_xid != 0 {
                        child != vok_xid
                    } else {
                        geom_outside
                    };
                    return Some((geom_outside || window_outside, x, y, child));
                }
            }
            None
        }).flatten()
    }

    /// 检测 GNOME Shell 是否正处于 Overview 概览模式 (如用户按下了 Super / 开始键呼出系统全览)
    pub fn is_gnome_overview_active() -> bool {
        let Ok(conn) = zbus::blocking::Connection::session() else {
            return false;
        };
        let Ok(proxy) = zbus::blocking::Proxy::new(
            &conn,
            "org.gnome.Shell",
            "/org/gnome/Shell",
            "org.gnome.Shell",
        ) else {
            return false;
        };
        proxy.get_property::<bool>("OverviewActive").unwrap_or(false)
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

    /// 获取当前 Vok Overlay 窗口的 XID
    pub fn get_vok_window_id() -> Option<u32> {
        Self::with_cached_conn(|conn, screen_num| {
            let screen = &conn.setup().roots[screen_num];
            Self::find_window_by_title(conn, screen.root, "Vok Overlay").ok().flatten()
        }).flatten()
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
                    info!("【任务栏隐形与工作区贴合】已成功对窗口 [{}] (XID: 0x{:x}) 设置 SKIP_TASKBAR 和 MAXIMIZED", title, win);
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

    fn check_window_title(
        conn: &impl Connection,
        win: x11rb::protocol::xproto::Window,
        title: &str,
        net_wm_name_atom: u32,
        wm_name_atom: u32,
    ) -> Result<Option<x11rb::protocol::xproto::Window>, WindowError> {
        if let Ok(prop) = conn.get_property(false, win, net_wm_name_atom, AtomEnum::ANY, 0, 1024) {
            if let Ok(reply) = prop.reply() {
                if !reply.value.is_empty() {
                    if let Ok(s) = String::from_utf8(reply.value) {
                        if s == title {
                            return Ok(Some(win));
                        }
                    }
                }
            }
        }
        if let Ok(prop2) = conn.get_property(false, win, wm_name_atom, AtomEnum::ANY, 0, 1024) {
            if let Ok(reply) = prop2.reply() {
                if !reply.value.is_empty() {
                    if let Ok(s) = String::from_utf8(reply.value) {
                        if s == title {
                            return Ok(Some(win));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn find_window_by_title(
        conn: &impl Connection,
        root: x11rb::protocol::xproto::Window,
        title: &str,
    ) -> Result<Option<x11rb::protocol::xproto::Window>, WindowError> {
        let tree = conn.query_tree(root)?.reply()?;
        let net_wm_name_atom = conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
        let wm_name_atom: u32 = AtomEnum::WM_NAME.into();

        for &child in tree.children.iter().rev() {
            if let Some(w) = Self::check_window_title(conn, child, title, net_wm_name_atom, wm_name_atom)? {
                return Ok(Some(w));
            }
            if let Ok(subtree) = conn.query_tree(child) {
                if let Ok(subreply) = subtree.reply() {
                    for &subchild in subreply.children.iter().rev() {
                        if let Some(w) = Self::check_window_title(conn, subchild, title, net_wm_name_atom, wm_name_atom)? {
                            return Ok(Some(w));
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
            ClientMessageData, ClientMessageEvent, EventMask, CLIENT_MESSAGE_EVENT,
        };

        let net_wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let skip_taskbar = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_TASKBAR")?.reply()?.atom;
        let skip_pager = conn.intern_atom(false, b"_NET_WM_STATE_SKIP_PAGER")?.reply()?.atom;

        let event_mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;

        // 发送 ClientMessage 通知窗口管理器附加跳过任务栏和跳过分页器
        // 遵循 EWMH 标准：绝对不使用 change_property(REPLACE) 覆盖 _NET_WM_STATE，亦不破坏 GTK 原生最大化
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
        conn.flush()?;

        Ok(())
    }

    /// 呼出唤起时，向窗口管理器申请将指定窗口提升到最前台并激活
    pub fn raise_and_activate_window_by_title(title: &str) -> bool {
        Self::with_cached_conn(|conn, screen_num| {
            let screen = &conn.setup().roots[screen_num];
            match Self::find_window_by_title(conn, screen.root, title) {
                Ok(Some(win)) => {
                    let res = Self::activate_window(conn, screen.root, win);
                    if res.is_ok() {
                        info!("【最前台提升】已向窗口管理器申请激活提升 [{}] (0x{:x})", title, win);
                        true
                    } else {
                        warn!("提升激活窗口失败: {:?}", res);
                        false
                    }
                }
                Ok(None) => {
                    warn!("未在 X11 窗口树中找到标题为 [{}] 的窗口以进行激活", title);
                    false
                }
                Err(e) => {
                    warn!("遍历 X11 窗口树失败: {:?}", e);
                    false
                }
            }
        }).unwrap_or(false)
    }

    fn activate_window(
        conn: &impl Connection,
        root: x11rb::protocol::xproto::Window,
        win: x11rb::protocol::xproto::Window,
    ) -> Result<(), WindowError> {
        use x11rb::protocol::xproto::{
            ClientMessageData, ClientMessageEvent, EventMask, CLIENT_MESSAGE_EVENT,
        };

        let net_active_window = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
            .reply()?
            .atom;

        let event_mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
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
        conn.flush()?;

        Ok(())
    }
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};

/// 全局工作区外点击监听器 (基于 XInput2 RawButtonPress，无侵入，不独占抓取，零按键干扰，零鼠标晃动误触发)
pub struct OutsideClickWatcher {
    running: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    workarea: Arc<RwLock<WorkAreaGeometry>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl OutsideClickWatcher {
    /// 启动后台监听线程
    pub fn start<F: Fn() + Send + Sync + 'static>(
        initial_workarea: WorkAreaGeometry,
        on_outside_click: F,
    ) -> Result<Self, WindowError> {
        let running = Arc::new(AtomicBool::new(true));
        let enabled = Arc::new(AtomicBool::new(false));
        let workarea = Arc::new(RwLock::new(initial_workarea));

        let running_clone = running.clone();
        let enabled_clone = enabled.clone();
        let workarea_clone = workarea.clone();

        let thread_handle = thread::Builder::new()
            .name("vok-outside-click".to_string())
            .spawn(move || {
                let Ok((conn, screen_num)) = x11rb::connect(None) else {
                    tracing::warn!("OutsideClickWatcher 无法连接 X11");
                    return;
                };
                let root = conn.setup().roots[screen_num].root;

                use x11rb::protocol::xinput::{self, ConnectionExt as _};
                if conn.xinput_xi_query_version(2, 2).is_err() {
                    tracing::warn!("X Server 不支持 XInput2 2.2，无法启用原生外置点击感知");
                    return;
                }

                // 监听全局所有物理主指针的 RawButtonPress (deviceid = 1 为 XIAllMasterDevices)
                let event_mask = xinput::EventMask {
                    deviceid: 1,
                    mask: vec![xinput::XIEventMask::RAW_BUTTON_PRESS],
                };
                if let Err(e) = conn.xinput_xi_select_events(root, &[event_mask]) {
                    tracing::warn!("选择 XI2 事件失败: {:?}", e);
                    return;
                }
                let _ = conn.flush();

                while running_clone.load(Ordering::Relaxed) {
                    // 当处于未激活状态（如 Vok 处于隐藏状态或拖出透明代理时），低频休眠
                    if !enabled_clone.load(Ordering::Relaxed) {
                        thread::sleep(std::time::Duration::from_millis(50));
                        // 清理积压的旧事件
                        while let Ok(Some(_)) = conn.poll_for_event() {}
                        continue;
                    }

                    let mut had_event = false;
                    while let Ok(Some(ev)) = conn.poll_for_event() {
                        had_event = true;
                        if let x11rb::protocol::Event::XinputRawButtonPress(rbp) = ev {
                            // 仅响应物理鼠标按键点击 (1: 左键, 2: 中键, 3: 右键)
                            if rbp.detail == 1 || rbp.detail == 2 || rbp.detail == 3 {
                                if !enabled_clone.load(Ordering::Relaxed) {
                                    continue;
                                }
                                if let Ok(cookie) = conn.query_pointer(root) {
                                    if let Ok(ptr) = cookie.reply() {
                                        let current_wa = *workarea_clone.read().unwrap();
                                        let x = ptr.root_x as i32;
                                        let y = ptr.root_y as i32;
                                        let is_outside = x < current_wa.x
                                            || y < current_wa.y
                                            || x >= (current_wa.x + current_wa.width as i32)
                                            || y >= (current_wa.y + current_wa.height as i32);

                                        if is_outside {
                                            tracing::info!(
                                                "【全局外置点击感知】捕获到工作区外部物理点击 (x={}, y={}, 当前工作区={}x{}+{}+{}), 触发自动隐藏",
                                                x, y,
                                                current_wa.width, current_wa.height, current_wa.x, current_wa.y
                                            );
                                            // 自动禁用避免短时重复抖动
                                            enabled_clone.store(false, Ordering::Relaxed);
                                            on_outside_click();
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if !had_event {
                        thread::sleep(std::time::Duration::from_millis(15));
                    }
                }
            })
            .map_err(|e| WindowError::Connect(x11rb::errors::ConnectError::IoError(e)))?;

        Ok(Self {
            running,
            enabled,
            workarea,
            thread_handle: Some(thread_handle),
        })
    }

    /// 更新当前生效的工作区几何
    pub fn update_workarea(&self, geom: WorkAreaGeometry) {
        if let Ok(mut lock) = self.workarea.write() {
            *lock = geom;
        }
    }

    /// 启用/禁用点击检测
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

impl Drop for OutsideClickWatcher {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
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

    #[test]
    fn test_is_pointer_clicking_outside_does_not_panic() {
        let geom = WindowAdapter::get_work_area();
        // 查询不应 panic
        let _ = WindowAdapter::is_pointer_clicking_outside(geom);
    }

    #[test]
    fn test_raise_and_activate_window_does_not_panic() {
        let _ = WindowAdapter::raise_and_activate_window_by_title("NonExistentWindow");
    }

    #[test]
    fn test_is_gnome_overview_active_does_not_panic() {
        let _ = WindowAdapter::is_gnome_overview_active();
    }

    #[test]
    fn test_outside_click_watcher_lifecycle() {
        let geom = WindowAdapter::get_work_area();
        let clicked = Arc::new(AtomicBool::new(false));
        let clicked_clone = clicked.clone();
        let watcher = OutsideClickWatcher::start(geom, move || {
            clicked_clone.store(true, Ordering::Relaxed);
        });
        assert!(watcher.is_ok());
        let watcher = watcher.unwrap();
        watcher.update_workarea(geom);
        watcher.set_enabled(true);
        watcher.set_enabled(false);
    }

    #[test]
    fn test_xi2_raw_button_press() {
        WindowAdapter::with_cached_conn(|conn, screen_num| {
            let screen = &conn.setup().roots[screen_num];
            use x11rb::protocol::xinput::{self, ConnectionExt as _};
            use x11rb::protocol::xtest::ConnectionExt as _;

            let version = conn.xinput_xi_query_version(2, 2).unwrap().reply().unwrap();
            println!("XI2 version supported: {}.{}", version.major_version, version.minor_version);

            // Select XI_RawButtonPress on the root window for all master devices (deviceid = 1)
            let event_mask = xinput::EventMask {
                deviceid: 1,
                mask: vec![xinput::XIEventMask::RAW_BUTTON_PRESS],
            };
            conn.xinput_xi_select_events(screen.root, &[event_mask]).unwrap().check().unwrap();
            conn.flush().unwrap();

            // Release button 1 first
            let _ = conn.xtest_fake_input(5, 1, 0, screen.root, 0, 0, 0);
            conn.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));

            // Warp pointer to (50, 500) and click
            conn.warp_pointer(x11rb::NONE, screen.root, 0, 0, 0, 0, 50, 500).unwrap();
            conn.xtest_fake_input(4, 1, 0, screen.root, 0, 0, 0).unwrap();
            conn.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));
            conn.xtest_fake_input(5, 1, 0, screen.root, 0, 0, 0).unwrap();
            conn.flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(50));

            let mut received_raw = false;
            while let Ok(Some(ev)) = conn.poll_for_event() {
                if let x11rb::protocol::Event::XinputRawButtonPress(rbp) = ev {
                    let ptr = conn.query_pointer(screen.root).unwrap().reply().unwrap();
                    println!("XI2 RAW BUTTON PRESS: detail={}, pointer=({}, {})", rbp.detail, ptr.root_x, ptr.root_y);
                    received_raw = true;
                }
            }
            println!("Received XI2 RawButtonPress? {}", received_raw);
            assert!(received_raw, "Should receive XI2 RawButtonPress");

            use std::os::unix::io::AsRawFd;
            let fd = conn.stream().as_raw_fd();
            println!("X11 connection raw fd: {}", fd);
            assert!(fd >= 0);
        });
    }
}






