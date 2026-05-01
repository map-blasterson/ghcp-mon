//! Tests for ghcp_mon::ingest helpers. LLRs:
//! - Each envelope persisted as own raw record
//! - OTLP attribute flattening
//! - OTLP int value parsed as int64
//! - Raw request body persisted verbatim per OTLP request
//! - Replay parser tags envelopes by type
//! - Replay reader skips blank lines
//! - Replay reader skips unparseable lines

use ghcp_mon::ingest::{
    flatten_otlp_attributes, ingest_envelope, ingest_jsonl_file, otlp_metrics_to_envelopes,
    otlp_traces_to_envelopes, parse_file_exporter_line, persist_raw_request,
};
use ghcp_mon::model::Envelope;
use ghcp_mon::server::AppState;
use ghcp_mon::ws::Broadcaster;
use ghcp_mon::{db, error::AppError};
use serde_json::json;
use std::sync::Arc;

async fn fresh_state() -> AppState {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-ingest-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pool = db::open(&dir.join("test.db")).await.unwrap();
    AppState {
        pool,
        bus: Broadcaster::new(64),
        session_state_dir_override: Arc::new(None),
    }
}

#[test]
fn parse_file_exporter_line_tags_span_metric_log_variants() {
    let span = parse_file_exporter_line(r#"{"type":"span","traceId":"a","spanId":"b","name":"x","startTime":1}"#).expect("span");
    assert!(matches!(span, Envelope::Span(_)));
    let metric = parse_file_exporter_line(r#"{"type":"metric","name":"m","dataPoints":[]}"#).expect("metric");
    assert!(matches!(metric, Envelope::Metric(_)));
    let log = parse_file_exporter_line(r#"{"type":"log","body":"hi"}"#).expect("log");
    assert!(matches!(log, Envelope::Log(_)));
}

#[test]
fn parse_file_exporter_line_returns_bad_request_on_garbage() {
    let err = parse_file_exporter_line("not json at all").unwrap_err();
    match err {
        AppError::BadRequest(_) => {}
        other => panic!("expected BadRequest, got {:?}", other),
    }
}

#[test]
fn flatten_otlp_attributes_unwraps_scalars_and_recurses() {
    let arr = json!([
        {"key":"s","value":{"stringValue":"hello"}},
        {"key":"i","value":{"intValue":"42"}},
        {"key":"d","value":{"doubleValue":3.5}},
        {"key":"b","value":{"boolValue":true}},
        {"key":"a","value":{"arrayValue":{"values":[
            {"stringValue":"x"},{"intValue":"7"}
        ]}}},
        {"key":"kv","value":{"kvlistValue":{"values":[
            {"key":"inner","value":{"stringValue":"v"}}
        ]}}},
    ]);
    let m = flatten_otlp_attributes(&arr);
    assert_eq!(m.get("s"), Some(&json!("hello")));
    assert_eq!(m.get("i"), Some(&json!(42_i64)));
    assert!(m.get("i").unwrap().is_i64(), "intValue MUST be JSON number i64");
    assert_eq!(m.get("d"), Some(&json!(3.5)));
    assert_eq!(m.get("b"), Some(&json!(true)));
    let a = m.get("a").unwrap();
    assert!(a.is_array(), "arrayValue MUST recurse to array");
    let kv = m.get("kv").unwrap();
    assert!(kv.is_object(), "kvlistValue MUST recurse to object");
    assert_eq!(kv.get("inner"), Some(&json!("v")));
}

#[test]
fn flatten_otlp_attributes_int_value_unparseable_passes_through() {
    let arr = json!([{"key":"k","value":{"intValue":"not-a-number"}}]);
    let m = flatten_otlp_attributes(&arr);
    // MAY pass through unchanged; we only assert it does NOT panic and returns *some* value.
    assert!(m.contains_key("k"));
}

#[test]
fn flatten_otlp_attributes_empty_array_yields_empty_map() {
    let m = flatten_otlp_attributes(&json!([]));
    assert!(m.is_empty());
}

#[tokio::test]
async fn persist_raw_request_writes_verbatim_body() {
    let state = fresh_state().await;
    let body = "  { \"weird\": [1, 2,   3]  }  ";
    let id = persist_raw_request(&state.pool, "otlp-http-json", Some("application/json"), "otlp-traces", body)
        .await
        .unwrap();
    let row: (String, String, Option<String>, String) =
        sqlx::query_as("SELECT source, record_type, content_type, body FROM raw_records WHERE id = ?")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(row.0, "otlp-http-json");
    assert_eq!(row.1, "otlp-traces");
    assert_eq!(row.2.as_deref(), Some("application/json"));
    assert_eq!(row.3, body, "body MUST be persisted verbatim");
}

#[tokio::test]
async fn ingest_envelope_writes_one_raw_record_per_envelope() {
    let state = fresh_state().await;
    let line = r#"{"type":"span","traceId":"t1","spanId":"s1","name":"x","startTime":1}"#;
    let env = parse_file_exporter_line(line).unwrap();
    let id = ingest_envelope(&state, "test", line, env).await.unwrap();
    let row: (String, String, Option<String>, String) =
        sqlx::query_as("SELECT source, record_type, content_type, body FROM raw_records WHERE id = ?")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert_eq!(row.0, "test");
    assert_eq!(row.1, "span", "record_type MUST be the envelope type tag");
    assert_eq!(row.2.as_deref(), Some("application/json"));
    assert_eq!(row.3, line, "body MUST be the supplied raw_text verbatim");

    // A second call MUST insert a *new* raw_records row, not reuse.
    let id2 = ingest_envelope(&state, "test", line, parse_file_exporter_line(line).unwrap()).await.unwrap();
    assert_ne!(id, id2, "each envelope MUST get its own raw_records row");
}

#[tokio::test]
async fn ingest_jsonl_file_skips_blank_and_unparseable_lines() {
    let state = fresh_state().await;
    let dir = std::env::temp_dir().join(format!(
        "ghcp-mon-jsonl-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("file.jsonl");
    let contents = concat!(
        r#"{"type":"span","traceId":"a","spanId":"b","name":"x","startTime":1}"#, "\n",
        "\n",
        "   \n",
        "not-json-at-all\n",
        r#"{"type":"metric","name":"m","dataPoints":[]}"#, "\n",
    );
    std::fs::write(&path, contents).unwrap();

    let count = ingest_jsonl_file(&state, &path, "replay").await.unwrap();
    assert_eq!(count, 2, "MUST count only the two valid envelopes");

    let raw_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM raw_records WHERE source='replay'")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(raw_count, 2, "MUST persist exactly one raw_records row per ingested envelope");
}

#[test]
fn otlp_traces_to_envelopes_basic_shape() {
    let body = json!({
        "resourceSpans":[{"scopeSpans":[{"spans":[
            {"traceId":"t","spanId":"s","name":"chat",
             "kind":2,"startTimeUnixNano":"1000","endTimeUnixNano":"2000",
             "attributes":[{"key":"k","value":{"stringValue":"v"}}],
             "events":[],"status":{"code":0}},
            {"traceId":"t","spanId":"s2","parentSpanId":"","name":"x","kind":0,
             "startTimeUnixNano":1, "attributes":[], "events":[]}
        ]}]}]
    });
    let envs = otlp_traces_to_envelopes(&body);
    assert_eq!(envs.len(), 2);
    assert_eq!(envs[0].kind_tag, "span");
    assert_eq!(envs[0].trace_id, "t");
    assert_eq!(envs[0].span_id, "s");
    assert_eq!(envs[1].parent_span_id, None, "empty parentSpanId MUST drop to None");
}

#[test]
fn otlp_metrics_to_envelopes_collects_data_points() {
    let body = json!({
        "resourceMetrics":[{"scopeMetrics":[{"metrics":[
            {"name":"foo","gauge":{"dataPoints":[
                {"asDouble":1.0,"timeUnixNano":"100"}
            ]}}
        ]}]}]
    });
    let envs = otlp_metrics_to_envelopes(&body);
    assert_eq!(envs.len(), 1);
    assert_eq!(envs[0].name, "foo");
    assert_eq!(envs[0].data_points.len(), 1);
}
