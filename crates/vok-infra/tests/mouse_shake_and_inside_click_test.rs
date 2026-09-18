use std::thread::sleep;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::xtest::ConnectionExt as _;

#[test]
fn test_mouse_shaking_and_outside_click_verification() {
    let Ok((conn, screen_num)) = x11rb::connect(None) else {
        println!("No X11 display available, skipping test");
        return;
    };
    let screen = &conn.setup().roots[screen_num];

    // Ensure button 1 is released
    let _ = conn.xtest_fake_input(5, 1, 0, screen.root, 0, 0, 0);
    let _ = conn.flush();
    sleep(Duration::from_millis(30));

    // 1. Mouse shaking test: rapid movement inside and near boundary without clicks
    for i in 0..30 {
        let x = if i % 2 == 0 { 100 } else { 200 };
        let y = 300 + (i % 300);
        let _ = conn.warp_pointer(x11rb::NONE, screen.root, 0, 0, 0, 0, x, y);
        let _ = conn.flush();
        sleep(Duration::from_millis(5));
    }
    sleep(Duration::from_millis(50));

    // 2. Click outside on dock
    let _ = conn.warp_pointer(x11rb::NONE, screen.root, 0, 0, 0, 0, 50, 500);
    let _ = conn.xtest_fake_input(4, 1, 0, screen.root, 0, 0, 0);
    let _ = conn.flush();
    sleep(Duration::from_millis(20));
    let _ = conn.xtest_fake_input(5, 1, 0, screen.root, 0, 0, 0);
    let _ = conn.flush();
    sleep(Duration::from_millis(50));
}
