use evdev::Key;
use gtk4::glib;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use vok_app::app::VokApp;
use vok_infra::input::{EvdevMonitor, MockInputMonitor};

fn main() -> glib::ExitCode {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("设置默认日志器失败");

    info!("=== Vok (Linux) 效率工作台启动 ===");

    // 0. 极速路径：若传入 --toggle，通过 D-Bus 毫秒级触发正在运行的常驻守护进程并立即退出
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--toggle" || a == "-t") {
        let status = std::process::Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "com.vok.desktop",
                "--object-path",
                "/com/vok/desktop",
                "--method",
                "org.gtk.Actions.Activate",
                "toggle",
                "[]",
                "{}",
            ])
            .status();

        if let Ok(s) = status {
            if s.success() {
                return glib::ExitCode::SUCCESS;
            }
        }
    }

    // 0.1 若传入 --settings / -s，通过 D-Bus 唤起偏好设置窗口并立即退出
    if args.iter().any(|a| a == "--settings" || a == "-s") {
        let status = std::process::Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "com.vok.desktop",
                "--object-path",
                "/com/vok/desktop",
                "--method",
                "org.gtk.Actions.Activate",
                "settings",
                "[]",
                "{}",
            ])
            .status();

        if let Ok(s) = status {
            if s.success() {
                return glib::ExitCode::SUCCESS;
            }
        }
    }

    // 默认在 X11 / XWayland 下运行，确保 EWMH 任务栏隐形与工作区避让 100% 生效
    if std::env::var("GDK_BACKEND").is_err() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    let (event_rx, _evdev_handle, _mock_handle) = match EvdevMonitor::start(Key::KEY_F12) {
        Ok(monitor) => {
            info!("【硬件按键监听已就绪】成功独占监听 /dev/input 物理 F12 按键");
            let rx = monitor.event_receiver();
            (rx, Some(monitor), None)
        }
        Err(e) => {
            warn!("未能接入硬件 evdev (原因: {:?})", e);
            warn!("启动 Mock 仿真输入模式：系统将在 1.5 秒后自动模拟按键呼出以供验证");
            let mock = MockInputMonitor::new();
            let rx = mock.event_receiver();


            let mock_tx_stdin = mock.clone();
            std::thread::spawn(move || {
                use std::io::BufRead;
                let stdin = std::io::stdin();
                info!("【人工测试提示】终端按回车键 (Enter) 即可随时再次呼出 Vok 工作台，输入 q 退出");
                for line in stdin.lock().lines().map_while(Result::ok) {
                    let cmd = line.trim();
                    if cmd == "q" || cmd == "quit" {
                        std::process::exit(0);
                    }
                    info!("【终端触发】模拟呼出快捷键 (F12)...");
                    mock_tx_stdin.simulate_press(88);
                }
            });

            (rx, None, Some(mock))
        }
    };

    let app = VokApp::new(event_rx);
    app.run()
}
