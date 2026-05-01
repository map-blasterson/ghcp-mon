//! Tests for OTLP HTTP handlers. LLRs:
//! - OTLP rejects protobuf content type
//! - OTLP traces persists raw and normalizes envelopes
//! - OTLP metrics persists raw and normalizes envelopes
//! - OTLP logs persisted raw only

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use ghcp_mon::server::{otlp_router, AppState};
use ghcp_mon::ws::Broadcaster;
use ghcp_mon::db;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

async fn fresh_state() -> AppState {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-otlp-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pool = db::open(&dir.join("test.db")).await.unwrap();
    AppState { pool, bus: Broadcaster::new(64), session_state_dir_override: Arc::new(None) }
}

async fn read_json(resp: axum::response::Response) -> (StatusCode, Value) {
    let s = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (s, v)
}

#[tokio::test]
async fn protobuf_content_type_returns_501_on_traces() {
    let state = fresh_state().await;
    let app = otlp_router(state);
    let req = Request::builder()
        .method("POST")
        .uri("/v1/traces")
        .header("content-type", "application/x-protobuf")
        .body(Body::from(vec![0u8, 1, 2]))
        .unwrap();
    let (status, body) = read_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert!(body["error"].as_str().unwrap_or("").contains("not implemented"));
}

#[tokio::test]
async fn protobuf_content_type_returns_501_on_metrics_and_logs() {
    for path in ["/v1/metrics", "/v1/logs"] {
        let state = fresh_state().await;
        let app = otlp_router(state);
        let req = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "Application/X-Protobuf; charset=utf-8")
            .body(Body::from(vec![0u8]))
            .unwrap();
        let (status, _b) = read_json(app.oneshot(req).await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{} MUST reject protobuf", path);
    }
}

#[tokio::test]
async fn traces_persists_raw_and_normalizes_envelopes() {
    let state = fresh_state().await;
    let app = otlp_router(state.clone());
    let body_str = json!({
        "resourceSpans":[{"scopeSpans":[{"spans":[
            {"traceId":"t1","spanId":"s1","name":"chat","kind":2,
             "startTimeUnixNano":"1","endTimeUnixNano":"2",
             "attributes":[{"key":"gen_ai.conversation.id","value":{"stringValue":"c1"}}],
             "events":[]},
            {"traceId":"t1","spanId":"s2","name":"chat","kind":2,
             "startTimeUnixNano":"3","endTimeUnixNano":"4",
             "attributes":[{"key":"gen_ai.conversation.id","value":{"stringValue":"c1"}}],
             "events":[]}
        ]}]}]
    }).to_string();
    let req = Request::builder()
        .method("POST").uri("/v1/traces").header("content-type","application/json")
        .body(Body::from(body_str.clone())).unwrap();
    let (status, body) = read_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["accepted"], json!(2));
    assert!(body["partialSuccess"].is_object());

    // Exactly one raw_records row with record_type='otlp-traces' and verbatim body.
    let raw_traces: Vec<(String, String)> =
        sqlx::query_as("SELECT body, source FROM raw_records WHERE record_type='otlp-traces'")
            .fetch_all(&state.pool).await.unwrap();
    assert_eq!(raw_traces.len(), 1);
    assert_eq!(raw_traces[0].0, body_str, "raw body MUST be verbatim");
    assert_eq!(raw_traces[0].1, "otlp-http-json");

    // Two per-envelope rows with record_type='span'.
    let span_raw: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM raw_records WHERE record_type='span'")
        .fetch_one(&state.pool).await.unwrap();
    assert_eq!(span_raw, 2);
}

#[tokio::test]
async fn metrics_persists_raw_and_normalizes_envelopes() {
    let state = fresh_state().await;
    let app = otlp_router(state.clone());
    let body = json!({
        "resourceMetrics":[{"scopeMetrics":[{"metrics":[
            {"name":"m1","gauge":{"dataPoints":[{"asDouble":1.0,"timeUnixNano":"1"}]}},
            {"name":"m2","gauge":{"dataPoints":[{"asDouble":2.0,"timeUnixNano":"2"}]}}
        ]}]}]
    }).to_string();
    let req = Request::builder()
        .method("POST").uri("/v1/metrics").header("content-type","application/json")
        .body(Body::from(body)).unwrap();
    let (status, resp) = read_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["accepted"], json!(2));

    let raw_meta: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM raw_records WHERE record_type='otlp-metrics'")
        .fetch_one(&state.pool).await.unwrap();
    assert_eq!(raw_meta, 1);
    let metric_raw: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM raw_records WHERE record_type='metric'")
        .fetch_one(&state.pool).await.unwrap();
    assert_eq!(metric_raw, 2);
}

#[tokio::test]
async fn logs_persisted_raw_only_no_normalized_rows() {
    let state = fresh_state().await;
    let app = otlp_router(state.clone());
    let body = json!({"resourceLogs":[{"scopeLogs":[{"logRecords":[{"body":{"stringValue":"hi"}}]}]}]})
        .to_string();
    let req = Request::builder()
        .method("POST").uri("/v1/logs").header("content-type","application/json")
        .body(Body::from(body)).unwrap();
    let (status, resp) = read_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["accepted"], json!(0));

    let raw_logs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM raw_records WHERE record_type='otlp-logs'")
        .fetch_one(&state.pool).await.unwrap();
    assert_eq!(raw_logs, 1, "exactly one raw row with record_type='otlp-logs'");
    let derived: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM raw_records WHERE record_type='log'")
        .fetch_one(&state.pool).await.unwrap();
    assert_eq!(derived, 0, "logs MUST NOT produce normalized envelope rows");
}
