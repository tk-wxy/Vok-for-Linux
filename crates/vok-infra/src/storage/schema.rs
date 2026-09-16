use rusqlite::{Connection, Result};
use tracing::info;

/// 为 SQLite 连接配置 WAL 模式与并发性能参数
pub fn configure_connection(conn: &Connection) -> Result<()> {
    // 强制启用 WAL 模式 (支持并发无锁读 + 单写者)
    let journal_mode: String = conn.query_row("PRAGMA journal_mode = WAL;", [], |row| row.get(0))?;
    info!("SQLite journal_mode 已配置为: {}", journal_mode);

    // 设置同步级别为 NORMAL (WAL 模式下兼顾高吞吐与安全性)
    conn.execute_batch(
        "
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        ",
    )?;

    Ok(())
}

/// 执行数据库初始化与表结构迁移
pub fn initialize_schema(conn: &Connection) -> Result<()> {
    configure_connection(conn)?;

    conn.execute_batch(
        "
        -- 1. 启动台表
        CREATE TABLE IF NOT EXISTS launcher_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            target TEXT NOT NULL,
            icon_name TEXT NOT NULL,
            order_index INTEGER NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_launcher_order ON launcher_items(order_index);

        -- 2. 中转站表 (引用型流动货架)
        CREATE TABLE IF NOT EXISTS stage_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL,
            uri TEXT NOT NULL,
            path TEXT NOT NULL,
            display_name TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            mime_type TEXT NOT NULL,
            mtime_sec INTEGER NOT NULL,
            thumbnail TEXT,
            pinned INTEGER NOT NULL DEFAULT 0,
            order_index INTEGER NOT NULL,
            created_at_ms INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_stage_order ON stage_items(order_index);
        CREATE INDEX IF NOT EXISTS idx_stage_path ON stage_items(path);
        ",
    )?;

    info!("SQLite schema 初始化与索引创建完成");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configure_and_initialize_schema() {
        let conn = Connection::open_in_memory().expect("open in memory db");
        initialize_schema(&conn).expect("initialize schema should succeed");

        // 验证表是否存在
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(tables.contains(&"launcher_items".to_string()));
        assert!(tables.contains(&"stage_items".to_string()));
    }
}
