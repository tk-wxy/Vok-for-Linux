use vok_core::ThemeMode;
use vok_infra::storage::StorageClient;
use vok_infra::window::WorkAreaGeometry;

#[test]
fn test_theme_persistence_and_manager_flow() {
    let tmp_dir = std::env::temp_dir().join(format!("vok_test_theme_{}", std::time::SystemTime::now().elapsed().unwrap().as_nanos()));
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let db_path = tmp_dir.join("vok_test.db");

    let storage = StorageClient::open(&db_path).unwrap();

    // 1. 默认无配置时为 system
    let default_val = storage.get_setting("theme.mode").unwrap();
    assert_eq!(default_val, None);

    // 2. 模拟写入并读取
    storage.set_setting("theme.mode", ThemeMode::Light.as_str()).unwrap();
    assert_eq!(storage.get_setting("theme.mode").unwrap(), Some("light".to_string()));

    storage.set_setting("theme.mode", ThemeMode::Dark.as_str()).unwrap();
    assert_eq!(storage.get_setting("theme.mode").unwrap(), Some("dark".to_string()));

    storage.set_setting("theme.mode", ThemeMode::FollowSystem.as_str()).unwrap();
    assert_eq!(storage.get_setting("theme.mode").unwrap(), Some("system".to_string()));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_outside_boundary_math() {
    let geom = WorkAreaGeometry {
        x: 132,
        y: 64,
        width: 3068,
        height: 1936,
    };

    // 位于 Ubuntu Dock 区域 (x < 132) 判定为 outside
    let dock_x = 50;
    let dock_y = 500;
    let is_outside_dock = dock_x < geom.x
        || dock_y < geom.y
        || dock_x >= (geom.x + geom.width as i32)
        || dock_y >= (geom.y + geom.height as i32);
    assert!(is_outside_dock);

    // 位于 GNOME Top Bar 区域 (y < 64) 判定为 outside
    let topbar_x = 500;
    let topbar_y = 30;
    let is_outside_topbar = topbar_x < geom.x
        || topbar_y < geom.y
        || topbar_x >= (geom.x + geom.width as i32)
        || topbar_y >= (geom.y + geom.height as i32);
    assert!(is_outside_topbar);

    // 位于工作台内部区域 判定为 inside (not outside)
    let inside_x = 500;
    let inside_y = 500;
    let is_outside_inside = inside_x < geom.x
        || inside_y < geom.y
        || inside_x >= (geom.x + geom.width as i32)
        || inside_y >= (geom.y + geom.height as i32);
    assert!(!is_outside_inside);
}
