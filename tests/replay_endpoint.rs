//! Tests for ghcp_mon::api::replay endpoint. LLR:
//! - Replay endpoint accepts path and returns count

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use ghcp_mon::db;
use ghcp_mon::server::{api_router, AppState};
use ghcp_mon::ws::Broadcaster;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

async fn fresh_state() -> AppState {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-replay-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pool = db::open(&dir.join("test.db")).await.unwrap();
    AppState { pool, bus: Broadcaster::new(64), session_state_dir_override: Arc::new(None) }
}

fn write_fixture(contents: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-replay-fixture-{}-{}", std::process::id(), n));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("file.jsonl");
    std::fs::write(&path, contents).unwrap();
    path
}

#[tokio::test]
async fn replay_endpoint_returns_path_and_count() {
    let state = fresh_state().await;
    let app = api_router(state.clone());
    let path = write_fixture(concat!(
        r#"{"type":"span","traceId":"t","spanId":"a","name":"x","startTime":1}"#, "\n",
        "\n",
        "garbage\n",
        r#"{"type":"metric","name":"m","dataPoints":[]}"#, "\n",
    ));
    let body_str = json!({"path": path.to_string_lossy()}).to_string();
    let req = Request::builder().method("POST").uri("/api/replay")
        .header("content-type", "application/json")
        .body(Body::from(body_str)).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["path"], json!(path.to_string_lossy()));
    assert_eq!(v["ingested"], json!(2));

    // Source 'replay' indicates the in-process JSONL path was used.
    let replay_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM raw_records WHERE source='replay'")
        .fetch_one(&state.pool).await.unwrap();
    assert_eq!(replay_rows, 2);
}
