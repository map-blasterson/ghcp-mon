//! Tests for db::open. LLRs:
//! - DB open creates parent directory
//! - DB open enables WAL and foreign keys
//! - DB open runs migrations

use ghcp_mon::db;

fn unique_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-db-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[tokio::test]
async fn open_creates_missing_parent_directory_recursively() {
    let root = unique_dir();
    let nested = root.join("a").join("b").join("c");
    let db_path = nested.join("test.db");
    assert!(!nested.exists(), "precondition: parent must not exist");
    let _pool = db::open(&db_path).await.expect("open ok");
    assert!(nested.exists(), "open MUST create the parent directory");
}

#[tokio::test]
async fn open_enables_wal_and_foreign_keys() {
    let dir = unique_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let pool = db::open(&dir.join("test.db")).await.unwrap();
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode;")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mode.to_lowercase(), "wal", "journal_mode MUST be WAL");
    let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys;")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(fk, 1, "foreign_keys MUST be on");
}

#[tokio::test]
async fn open_runs_migrations_so_tables_are_reachable() {
    let dir = unique_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let pool = db::open(&dir.join("test.db")).await.unwrap();
    // Tables created by migrations must be queryable post-open.
    for table in ["raw_records", "spans", "sessions", "chat_turns", "tool_calls"] {
        let sql = format!("SELECT COUNT(*) FROM {}", table);
        let n: i64 = sqlx::query_scalar(&sql).fetch_one(&pool).await
            .unwrap_or_else(|e| panic!("table {} unreachable: {}", table, e));
        assert!(n >= 0);
    }
}
