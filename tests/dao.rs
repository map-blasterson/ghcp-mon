//! Tests for DAO insert_raw. LLR:
//! - DAO insert_raw returns row id

use ghcp_mon::db;

fn unique_db_path() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-dao-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("test.db")
}

#[tokio::test]
async fn insert_raw_returns_row_id_and_persists_fields() {
    let pool = db::open(&unique_db_path()).await.unwrap();
    let id1 = db::dao::insert_raw(&pool, "src1", "rt1", Some("application/json"), "body1")
        .await
        .expect("insert ok");
    assert!(id1 >= 1, "row id must be >= 1");
    let id2 = db::dao::insert_raw(&pool, "src2", "rt2", None, "body2")
        .await
        .expect("insert ok");
    assert!(id2 > id1, "auto-increment must yield strictly larger id");

    let row: (String, String, Option<String>, String) = sqlx::query_as(
        "SELECT source, record_type, content_type, body FROM raw_records WHERE id = ?",
    )
    .bind(id1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "src1");
    assert_eq!(row.1, "rt1");
    assert_eq!(row.2.as_deref(), Some("application/json"));
    assert_eq!(row.3, "body1");

    let row2: (String, String, Option<String>, String) = sqlx::query_as(
        "SELECT source, record_type, content_type, body FROM raw_records WHERE id = ?",
    )
    .bind(id2)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row2.2, None, "None content_type round-trips as NULL");
}
