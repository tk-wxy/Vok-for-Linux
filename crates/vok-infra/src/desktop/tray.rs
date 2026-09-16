//! 系统托盘常驻服务模块 (StatusNotifierItem 规范)
//!
//! 遵循现代 Linux 桌面标准，通过 D-Bus StatusNotifierItem (SNI) 协议在系统顶栏常驻图标，
//! 支持左键单击 Toggle 显隐、右键上下文菜单（呼出/退出），实现无窗口静默守护。

use std::sync::Arc;
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::*;
use ksni::Tray;
use tracing::info;

/// Vok 系统托盘定义
pub struct VokTray {
    on_toggle: Arc<dyn Fn() + Send + Sync>,
    on_quit: Arc<dyn Fn() + Send + Sync>,
}

impl Tray for VokTray {
    fn id(&self) -> String {
        "vok-tray".into()
    }

    fn icon_name(&self) -> String {
        // 使用现代 Linux 桌面广泛内置的符号图标 (如 GNOME / Yaru / Papirus)
        "view-grid-symbolic".into()
    }

    fn title(&self) -> String {
        "Vok 效率工作台".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Vok 效率工作台".into(),
            description: "按 F12 呼出/收起 | 临时中转站与启动台".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        info!("【托盘激活】用户左键点击托盘图标，派发 Toggle 信号");
        (self.on_toggle)();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let toggle = self.on_toggle.clone();
        let quit = self.on_quit.clone();

        vec![
            StandardItem {
                label: "🚀 呼出 / 收起工作台 (F12)".into(),
                activate: Box::new(move |_| {
                    toggle();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "🚪 退出 Vok".into(),
                activate: Box::new(move |_| {
                    info!("【托盘退出】用户点击托盘菜单退出项");
                    quit();
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// 托盘常驻服务守护句柄
pub struct TrayHandle {
    #[allow(dead_code)]
    handle: Handle<VokTray>,
}

impl VokTray {
    /// 启动系统托盘常驻后台守护服务
    pub fn start(
        on_toggle: impl Fn() + Send + Sync + 'static,
        on_quit: impl Fn() + Send + Sync + 'static,
    ) -> Result<TrayHandle, Box<dyn std::error::Error + Send + Sync>> {
        let tray = Self {
            on_toggle: Arc::new(on_toggle),
            on_quit: Arc::new(on_quit),
        };
        let handle = tray.assume_sni_available(true).spawn()?;
        info!("【系统托盘就绪】StatusNotifierItem 托盘图标已注册到系统通知区域");
        Ok(TrayHandle { handle })
    }
}
