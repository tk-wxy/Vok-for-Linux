use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use vok_core::model::{calculate_reorder, LauncherItem, StageItem, StageItemKind};
use vok_infra::storage::StorageClient;

fn temp_db_path(name: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("vok_test_{}_{}.db", name, now));
    // 清理可能遗留的旧文件
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
    path
}

fn cleanup_temp_db(path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
}

#[test]
fn test_storage_client_open_and_wal_mode() {
    let db_path = temp_db_path("wal_mode");
    let client = StorageClient::open(&db_path).expect("open client");

    let conn = client
        .open_read_connection()
        .expect("open read connection");
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
        .expect("query journal_mode");

    assert_eq!(journal_mode.to_lowercase(), "wal");

    client.shutdown();
    cleanup_temp_db(&db_path);
}

#[test]
fn test_launcher_crud_and_reorder() {
    let db_path = temp_db_path("launcher_crud");
    let client = StorageClient::open(&db_path).expect("open client");

    // 1. 新增 3 个启动项
    let item1 = LauncherItem::new(
        0,
        "Firefox",
        "firefox.desktop",
        "firefox",
        0,
        1726000000000,
    );
    let item2 = LauncherItem::new(
        0,
        "Alacritty",
        "alacritty.desktop",
        "alacritty",
        1,
        1726000001000,
    );
    let item3 = LauncherItem::new(
        0,
        "Nautilus",
        "org.gnome.Nautilus.desktop",
        "system-file-manager",
        2,
        1726000002000,
    );

    let id1 = client.add_launcher_item(item1).expect("add item 1");
    let id2 = client.add_launcher_item(item2).expect("add item 2");
    let id3 = client.add_launcher_item(item3).expect("add item 3");

    assert!(id1 > 0 && id2 > id1 && id3 > id2);

    // 2. 查询列表
    let items = client.list_launcher_items().expect("list launcher items");
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].id, id1);
    assert_eq!(items[0].title, "Firefox");
    assert_eq!(items[1].id, id2);
    assert_eq!(items[2].id, id3);

    // 3. 拖拽重排 (将 index 0 的 Firefox 移动到末尾 index 2)
    let current_ids: Vec<i64> = items.iter().map(|it| it.id).collect();
    let reorder_changes =
        calculate_reorder(&current_ids, 0, 2).expect("calculate reorder");
    client
        .reorder_launcher_items(reorder_changes)
        .expect("reorder launcher items");

    let reordered_items = client.list_launcher_items().expect("list after reorder");
    assert_eq!(reordered_items[0].id, id2); // Alacritty 变成第一个
    assert_eq!(reordered_items[1].id, id3); // Nautilus 变成第二个
    assert_eq!(reordered_items[2].id, id1); // Firefox 变成第三个

    // 4. 删除项
    client.delete_launcher_item(id2).expect("delete item 2");
    let remaining_items = client.list_launcher_items().expect("list after delete");
    assert_eq!(remaining_items.len(), 2);
    assert_eq!(remaining_items[0].id, id3);
    assert_eq!(remaining_items[1].id, id1);

    client.shutdown();
    cleanup_temp_db(&db_path);
}

#[test]
fn test_stage_batch_insert_and_operations() {
    let db_path = temp_db_path("stage_batch");
    let client = StorageClient::open(&db_path).expect("open client");

    // 1. 生成 100 个模拟中转项
    let mut batch = Vec::with_capacity(100);
    for i in 0..100 {
        let path = PathBuf::from(format!("/tmp/doc_{}.pdf", i));
        let item = StageItem::new(
            0,
            StageItemKind::File,
            format!("file:///tmp/doc_{}.pdf", i),
            path,
            format!("doc_{}.pdf", i),
            (i as u64 + 1) * 1024,
            "application/pdf",
            1726000000 + i as u64,
            None,
            false,
            i,
            1726000000000 + i as i64,
        );
        batch.push(item);
    }

    // 2. 批量插入并测试耗时
    let start = Instant::now();
    let ids = client
        .add_stage_items_batch(batch)
        .expect("batch insert should succeed");
    let elapsed = start.elapsed();
    assert_eq!(ids.len(), 100);
    // 门禁指标：100条单事务批量插入应远低于 1 秒 (通常 < 50ms)
    assert!(
        elapsed < Duration::from_millis(500),
        "批量插入 100 条耗时过长: {:?}",
        elapsed
    );

    // 3. 查询
    let items = client.list_stage_items().expect("list stage items");
    assert_eq!(items.len(), 100);

    // 4. 更新缩略图
    let thumb_path = PathBuf::from("/tmp/thumb_42.png");
    client
        .update_stage_thumbnail(ids[42], thumb_path.clone())
        .expect("update thumbnail");

    // 5. 设置置顶 (pinned)
    client
        .set_stage_pinned(ids[50], true)
        .expect("pin item 50");

    let items_after_pin = client.list_stage_items().expect("list after pin");
    // 置顶项排在最前面
    assert_eq!(items_after_pin[0].id, ids[50]);
    assert!(items_after_pin[0].pinned);

    // 查找刚刚更新缩略图的项
    let item_42 = items_after_pin
        .iter()
        .find(|it| it.id == ids[42])
        .expect("find item 42");
    assert_eq!(item_42.thumbnail, Some(thumb_path));

    // 6. 清理所有未置顶项
    let deleted_count = client
        .clear_unpinned_stage_items()
        .expect("clear unpinned");
    assert_eq!(deleted_count, 99);

    let final_items = client.list_stage_items().expect("list final");
    assert_eq!(final_items.len(), 1);
    assert_eq!(final_items[0].id, ids[50]);

    client.shutdown();
    cleanup_temp_db(&db_path);
}

#[test]
fn test_concurrent_readers_and_writer() {
    let db_path = temp_db_path("concurrent_rw");
    let client = StorageClient::open(&db_path).expect("open client");

    let running = Arc::new(AtomicBool::new(true));
    let mut reader_handles = Vec::new();

    // 启动 4 个并发只读查询线程
    for _reader_id in 0..4 {
        let client_clone = client.clone();
        let running_clone = running.clone();
        let handle = thread::spawn(move || {
            let mut query_count = 0;
            while running_clone.load(Ordering::Relaxed) {
                let _stage_items = client_clone
                    .list_stage_items()
                    .expect("concurrent reader stage list should not lock");
                let _launcher_items = client_clone
                    .list_launcher_items()
                    .expect("concurrent reader launcher list should not lock");
                query_count += 1;
                // 轻微让出 CPU
                thread::yield_now();
            }
            query_count
        });
        reader_handles.push(handle);
    }

    // 主线程持续写入 30 批数据
    for batch_idx in 0..30 {
        let mut batch = Vec::new();
        for i in 0..5 {
            let item = StageItem::new(
                0,
                StageItemKind::File,
                format!("file:///tmp/f_{}_{}.txt", batch_idx, i),
                PathBuf::from(format!("/tmp/f_{}_{}.txt", batch_idx, i)),
                format!("f_{}_{}.txt", batch_idx, i),
                1024,
                "text/plain",
                1726000000,
                None,
                false,
                i,
                1726000000000,
            );
            batch.push(item);
        }
        client
            .add_stage_items_batch(batch)
            .expect("batch insert in writer");
        thread::sleep(Duration::from_millis(5));
    }

    // 停止读线程
    running.store(false, Ordering::Relaxed);
    for h in reader_handles {
        let queries = h.join().expect("reader join");
        assert!(queries > 0, "每个读线程至少成功执行多次查询");
    }

    client.shutdown();
    cleanup_temp_db(&db_path);
}

#[test]
fn test_r4_stage_never_touches_original_file() {
    let db_path = temp_db_path("r4_file_safety");
    let client = StorageClient::open(&db_path).expect("open client");

    // 创建一个真实存在的临时物理源文件
    let source_file_path = std::env::temp_dir().join("vok_test_original_user_file.txt");
    {
        let mut f = File::create(&source_file_path).expect("create source file");
        f.write_all(b"Original User Secret Content")
            .expect("write content");
    }

    let meta_before = fs::metadata(&source_file_path).expect("get metadata before");
    let mtime_before = meta_before.modified().expect("mtime before");
    let len_before = meta_before.len();

    // 录入到中转站
    let stage_item = StageItem::from_path_ref(
        0,
        &source_file_path,
        "vok_test_original_user_file.txt",
        len_before,
        "text/plain",
        1726000000,
        0,
        1726000000000,
    );

    let ids = client
        .add_stage_items_batch(vec![stage_item])
        .expect("add item");
    assert_eq!(ids.len(), 1);

    // 查询列表
    let items = client.list_stage_items().expect("list items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path, source_file_path);

    // 删除中转站记录
    client.delete_stage_item(ids[0]).expect("delete item");
    let items_after_del = client.list_stage_items().expect("list items");
    assert!(items_after_del.is_empty());

    // 【核心断言】原文件必须完好无损存在于物理磁盘上，内容与修改时间毫发无损！
    assert!(source_file_path.exists(), "原文件绝不能被删除");
    let meta_after = fs::metadata(&source_file_path).expect("get metadata after");
    assert_eq!(meta_after.len(), len_before, "文件大小不得改变");
    assert_eq!(
        meta_after.modified().expect("mtime after"),
        mtime_before,
        "文件修改时间不得改变"
    );

    let content_after = fs::read_to_string(&source_file_path).expect("read content");
    assert_eq!(content_after, "Original User Secret Content");

    // 清理测试物理文件
    let _ = fs::remove_file(&source_file_path);
    client.shutdown();
    cleanup_temp_db(&db_path);
}
