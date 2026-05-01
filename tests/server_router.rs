//! Tests for the Server router. LLRs:
//! - OTLP body limit 64 MiB
//! - OTLP router exposes traces metrics logs endpoints

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ghcp_mon::db;
use ghcp_mon::server::{otlp_router, AppState};
use ghcp_mon::ws::Broadcaster;
use std::sync::Arc;
use tower::ServiceExt;

async fn fresh_state() -> AppState {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-router-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pool = db::open(&dir.join("test.db")).await.unwrap();
    AppState { pool, bus: Broadcaster::new(64), session_state_dir_override: Arc::new(None) }
}

#[tokio::test]
async fn otlp_router_exposes_v1_traces() {
    let app = otlp_router(fresh_state().await);
    let req = Request::builder().method("POST").uri("/v1/traces")
        .header("content-type","application/json")
        .body(Body::from("{}")).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
    assert_ne!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn otlp_router_exposes_v1_metrics() {
    let app = otlp_router(fresh_state().await);
    let req = Request::builder().method("POST").uri("/v1/metrics")
        .header("content-type","application/json")
        .body(Body::from("{}")).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
    assert_ne!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn otlp_router_exposes_v1_logs() {
    let app = otlp_router(fresh_state().await);
    let req = Request::builder().method("POST").uri("/v1/logs")
        .header("content-type","application/json")
        .body(Body::from("{}")).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
    assert_ne!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn otlp_router_permits_body_just_under_64_mib() {
    // 64 MiB - 1 byte payload should NOT be rejected by the body-limit layer.
    // (It will be rejected later by JSON parsing, but at that stage
    // the body has already been accepted, so status should NOT be 413.)
    let app = otlp_router(fresh_state().await);
    let size = 64 * 1024 * 1024 - 1;
    let body = vec![b'a'; size];
    let req = Request::builder().method("POST").uri("/v1/traces")
        .header("content-type","application/json")
        .body(Body::from(body)).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_ne!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE,
        "bodies up to 64 MiB MUST be permitted");
}

#[tokio::test]
async fn otlp_router_rejects_body_over_64_mib() {
    // 64 MiB + 1 byte MUST be rejected by the DefaultBodyLimit layer (413).
    let app = otlp_router(fresh_state().await);
    let size = 64 * 1024 * 1024 + 1;
    let body = vec![b'a'; size];
    let req = Request::builder().method("POST").uri("/v1/traces")
        .header("content-type","application/json")
        .body(Body::from(body)).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
