use std::fs::{self, File};
use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossbeam_channel::bounded;
use image::{Rgb, RgbImage};
use vok_infra::desktop::{DesktopEntry, ThumbnailWorker, MAX_THUMB_SIZE};

#[test]
fn test_desktop_entry_file_parsing() {
    let temp_dir = std::env::temp_dir();
    let desktop_file_path = temp_dir.join(format!(
        "vok_test_app_{}.desktop",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));

    let content = r#"
[Desktop Entry]
Type=Application
Name=Vok Workstation
Name[zh_CN]=Vok 极速工作台
Name[zh]=Vok 效率桌面
Exec=/usr/bin/vok-app --daemon %F
Icon=vok-icon
Terminal=false
Categories=Utility;Workspace;
"#;
    fs::write(&desktop_file_path, content).expect("write desktop file");

    let entry = DesktopEntry::from_file(&desktop_file_path).expect("parse desktop file");
    // 默认如果未设置特殊环境变量，至少能解析出名字与清洗后的 Exec
    assert!(!entry.name.is_empty());
    assert_eq!(entry.exec, "/usr/bin/vok-app --daemon");
    assert_eq!(entry.icon, "vok-icon");
    assert!(!entry.terminal);
    assert!(!entry.no_display);
    assert_eq!(entry.categories, vec!["Utility", "Workspace"]);

    let _ = fs::remove_file(desktop_file_path);
}

#[test]
fn test_thumbnail_worker_async_pipeline() {
    let temp_dir = std::env::temp_dir();
    let source_img_path = temp_dir.join(format!(
        "vok_test_async_src_{}.png",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));

    // 生成一张 640x360 的测试图片
    let mut img = RgbImage::new(640, 360);
    for pixel in img.pixels_mut() {
        *pixel = Rgb([50, 120, 220]);
    }
    img.save(&source_img_path).expect("save source image");

    // 启动 Worker
    let worker = ThumbnailWorker::start().expect("start thumbnail worker");

    let (tx, rx) = bounded(1);
    worker.request_thumbnail(
        1001,
        source_img_path.clone(),
        1726000000,
        move |id, thumb_opt| {
            let _ = tx.send((id, thumb_opt));
        },
    );

    let (id, thumb_opt) = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("receive thumbnail result within 5s");

    assert_eq!(id, 1001);
    let thumb_path = thumb_opt.expect("thumbnail should be generated");
    assert!(thumb_path.exists());

    let loaded = image::open(&thumb_path).expect("open generated thumb");
    assert!(loaded.width() <= MAX_THUMB_SIZE);
    assert!(loaded.height() <= MAX_THUMB_SIZE);
    // 640x360 等比例缩放至 160 宽，高应为 90
    assert_eq!(loaded.width(), 160);
    assert_eq!(loaded.height(), 90);

    // 再次请求应直接命中缓存
    let cached = ThumbnailWorker::get_cached_path(&source_img_path, 1726000000);
    assert_eq!(cached, Some(thumb_path.clone()));

    worker.shutdown();
    let _ = fs::remove_file(source_img_path);
    let _ = fs::remove_file(thumb_path);
}

#[test]
fn test_thumbnail_quota_cleanup_logic() {
    let temp_dir = std::env::temp_dir();
    let quota_test_dir = temp_dir.join(format!(
        "vok_quota_test_{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    fs::create_dir_all(&quota_test_dir).expect("create quota test dir");

    // 模拟创建 52 个各 1MB 的伪缓存文件 (总计 52MB，超出 50MB 阈值)
    let chunk_1mb = vec![0u8; 1024 * 1024];
    for i in 0..52 {
        let fpath = quota_test_dir.join(format!("dummy_{:03}.png", i));
        let mut f = File::create(&fpath).expect("create dummy file");
        f.write_all(&chunk_1mb).expect("write dummy content");
    }

    // 执行清理
    ThumbnailWorker::enforce_disk_quota(&quota_test_dir);

    // 检查清理后总大小应降至 <= 40MB
    let mut remaining_bytes = 0u64;
    for entry in fs::read_dir(&quota_test_dir).unwrap().flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                remaining_bytes += meta.len();
            }
        }
    }

    assert!(
        remaining_bytes <= 40 * 1024 * 1024,
        "清理后体积仍超过 40MB: {} 字节",
        remaining_bytes
    );

    let _ = fs::remove_dir_all(quota_test_dir);
}

#[test]
fn test_desktop_shortcut_conflict_resolution() {
    use vok_infra::desktop::DesktopShortcut;

    let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if !current_desktop.contains("GNOME") && !current_desktop.contains("ubuntu") {
        return;
    }

    // 1. 模拟绑定 Alt+Space，验证 activate-window-menu 被设为 []
    DesktopShortcut::handle_system_conflicts("<Alt>space");
    let out1 = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.keybindings", "activate-window-menu"])
        .output()
        .expect("gsettings get");
    let text1 = String::from_utf8_lossy(&out1.stdout).trim().to_string();
    assert!(text1 == "@as []" || text1 == "[]", "activate-window-menu 应当被清空以避让冲突");

    // 2. 模拟切离 Alt+Space (如切换回 F12)，验证 activate-window-menu 被还原为默认值
    DesktopShortcut::handle_system_conflicts("F12");
    let out2 = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.keybindings", "activate-window-menu"])
        .output()
        .expect("gsettings get");
    let text2 = String::from_utf8_lossy(&out2.stdout).trim().to_string();
    assert!(text2.contains("<Alt>space"), "切离后应当自动还原系统原有快捷键");
}

#[test]
fn test_recording_conflict_lifecycle() {
    use vok_infra::desktop::DesktopShortcut;

    let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if !current_desktop.contains("GNOME") && !current_desktop.contains("ubuntu") {
        return;
    }

    // 1. 进入录入态：临时挂起 activate-window-menu
    DesktopShortcut::prepare_recording_conflicts();
    let out1 = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.keybindings", "activate-window-menu"])
        .output()
        .expect("gsettings get");
    let text1 = String::from_utf8_lossy(&out1.stdout).trim().to_string();
    assert!(text1 == "@as []" || text1 == "[]", "进入录入态应当立即清空 activate-window-menu");

    // 2. 取消录入 (Esc/失焦且当前是 F12)：恢复默认
    DesktopShortcut::restore_recording_conflicts("F12");
    let out2 = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.keybindings", "activate-window-menu"])
        .output()
        .expect("gsettings get");
    let text2 = String::from_utf8_lossy(&out2.stdout).trim().to_string();
    assert!(text2.contains("<Alt>space"), "取消录入且原键非 Alt+Space 应当还原系统菜单");

    // 3. 再次进入录入态并成功提交 Alt+Space：保持清空接管
    DesktopShortcut::prepare_recording_conflicts();
    DesktopShortcut::restore_recording_conflicts("<Alt>space");
    let out3 = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.wm.keybindings", "activate-window-menu"])
        .output()
        .expect("gsettings get");
    let text3 = String::from_utf8_lossy(&out3.stdout).trim().to_string();
    assert!(text3 == "@as []" || text3 == "[]", "提交 Alt+Space 应当继续保持接管清空状态");

    // 清理还原
    DesktopShortcut::handle_system_conflicts("F12");
}

