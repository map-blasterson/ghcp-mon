//! Span-canonical ingestion tests.

use ghcp_mon::{db, model::Envelope, normalize::{self, NormalizeCtx}, ingest, server::AppState, ws::Broadcaster};
use rand::seq::SliceRandom;
use rand::SeedableRng;

const FIXTURE: &str = "reference/copilot.log";

async fn fresh_state() -> AppState {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-test-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    let pool = db::open(&path).await.expect("open db");
    AppState { pool, bus: Broadcaster::new(64), session_state_dir_override: std::sync::Arc::new(None) }
}

fn fixture_lines(path: &str) -> Vec<String> {
    let body = std::fs::read_to_string(path).expect("read fixture");
    body.lines().filter(|l| !l.trim().is_empty()).map(|l| l.to_string()).collect()
}

fn span_lines(path: &str) -> Vec<String> {
    fixture_lines(path).into_iter().filter(|l| l.contains(r#""type":"span""#)).collect()
}

async fn ingest_lines(state: &AppState, lines: &[String]) {
    for line in lines {
        let env: Envelope = match ingest::parse_file_exporter_line(line) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let raw_id: i64 = sqlx::query_scalar(
            "INSERT INTO raw_records(source, record_type, body) VALUES('test','span',?) RETURNING id"
        ).bind(line).fetch_one(&state.pool).await.unwrap();
        let ctx = NormalizeCtx { pool: &state.pool, bus: &state.bus, raw_record_id: raw_id };
        normalize::handle_envelope(&ctx, &env).await.expect("normalize");
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct Snapshot {
    spans: i64,
    placeholders: i64,
    chat_turns: i64,
    tool_calls: i64,
    agent_runs: i64,
    sessions: i64,
    external_tool_calls: i64,
    hook_invocations: i64,
}

async fn snapshot(state: &AppState) -> Snapshot {
    let q = |sql: &'static str| async move {
        sqlx::query_scalar::<_, i64>(sql).fetch_one(&state.pool).await.unwrap()
    };
    Snapshot {
        spans: q("SELECT COUNT(*) FROM spans").await,
        placeholders: q("SELECT COUNT(*) FROM spans WHERE ingestion_state='placeholder'").await,
        chat_turns: q("SELECT COUNT(*) FROM chat_turns").await,
        tool_calls: q("SELECT COUNT(*) FROM tool_calls").await,
        agent_runs: q("SELECT COUNT(*) FROM agent_runs").await,
        sessions: q("SELECT COUNT(*) FROM sessions").await,
        external_tool_calls: q("SELECT COUNT(*) FROM external_tool_calls").await,
        hook_invocations: q("SELECT COUNT(*) FROM hook_invocations").await,
    }
}

#[tokio::test]
async fn replay_in_order_produces_complete_projections() {
    let state = fresh_state().await;
    let lines = span_lines(FIXTURE);
    assert!(lines.len() >= 5, "expected a multi-span fixture");
    ingest_lines(&state, &lines).await;
    let snap = snapshot(&state).await;
    assert_eq!(snap.placeholders, 0, "no placeholders should remain after full replay");
    assert!(snap.chat_turns >= 1);
    assert!(snap.tool_calls >= 1);
    assert!(snap.agent_runs >= 1, "should detect at least the root invoke_agent");
    assert!(snap.sessions >= 1);
}

#[tokio::test]
async fn shuffled_ingest_yields_identical_projection_counts() {
    let state_in_order = fresh_state().await;
    let lines = span_lines(FIXTURE);
    ingest_lines(&state_in_order, &lines).await;
    let snap_in_order = snapshot(&state_in_order).await;

    let state_shuffled = fresh_state().await;
    let mut shuffled = lines.clone();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0xC0FFEE);
    shuffled.shuffle(&mut rng);
    ingest_lines(&state_shuffled, &shuffled).await;
    let snap_shuffled = snapshot(&state_shuffled).await;

    assert_eq!(snap_in_order, snap_shuffled, "projection counts must be order-independent");
}

#[tokio::test]
async fn reverse_order_ingest_creates_then_upgrades_placeholders() {
    let state = fresh_state().await;
    // OTLP captures arrive in finish-order (children before their parents),
    // so the natural file order is already leaf-first. Ingesting the first
    // half should leave parents unseen → placeholder rows must appear.
    let lines = span_lines(FIXTURE);
    let half = lines.len() / 2;
    ingest_lines(&state, &lines[..half]).await;
    let mid = snapshot(&state).await;
    assert!(mid.placeholders > 0, "leaf-first ingest must create placeholder rows for unseen parents");
    // Ingest the rest; placeholders must all upgrade in place (no duplicates).
    ingest_lines(&state, &lines[half..]).await;
    let after = snapshot(&state).await;
    assert_eq!(after.placeholders, 0, "all placeholders must be upgraded");

    // Compare against full-replay baseline.
    let state_baseline = fresh_state().await;
    ingest_lines(&state_baseline, &lines).await;
    let baseline = snapshot(&state_baseline).await;
    assert_eq!(after, baseline, "split ingest must converge to the same state");
}

#[tokio::test]
async fn double_ingest_is_idempotent() {
    let state = fresh_state().await;
    let lines = span_lines(FIXTURE);
    ingest_lines(&state, &lines).await;
    let after_first = snapshot(&state).await;
    ingest_lines(&state, &lines).await;
    let after_second = snapshot(&state).await;
    assert_eq!(after_first, after_second, "re-ingesting the same spans must not create duplicates");
}

#[tokio::test]
async fn subagent_invoke_attaches_to_task_execute_tool() {
    let state = fresh_state().await;
    let lines = span_lines(FIXTURE);
    ingest_lines(&state, &lines).await;
    // Find the execute_tool task span.
    let task_span: Option<(i64, String, String)> = sqlx::query_as(
        "SELECT span_pk, trace_id, span_id FROM spans WHERE name = 'execute_tool task' LIMIT 1"
    ).fetch_optional(&state.pool).await.unwrap();
    let (task_pk, trace_id, task_span_id) = task_span.expect("fixture must contain an execute_tool task span");

    // Find at least one invoke_agent whose parent is the task span.
    let child_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM spans WHERE name LIKE 'invoke_agent%' AND trace_id = ? AND parent_span_id = ?"
    ).bind(&trace_id).bind(&task_span_id).fetch_one(&state.pool).await.unwrap();
    assert!(child_count >= 1, "subagent invoke_agent should be a child of execute_tool task");

    // The agent_run for the subagent must have parent_span_pk pointing at the task span.
    let attached: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_runs WHERE parent_span_pk = ?"
    ).bind(task_pk).fetch_one(&state.pool).await.unwrap();
    assert!(attached >= 1, "subagent agent_run must have parent_span_pk = task span_pk");
}

#[tokio::test]
async fn otlp_traces_endpoint_persists_raw() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::json;
    use tower::ServiceExt;

    let state = fresh_state().await;
    let app = ghcp_mon::server::otlp_router(state.clone());

    let body = json!({
        "resourceSpans": [{
            "resource": {"attributes": [{"key":"service.name","value":{"stringValue":"github-copilot"}}]},
            "scopeSpans": [{
                "scope": {"name":"github.copilot","version":"1.0.37"},
                "spans": [{
                    "traceId":"00000000000000000000000000000001",
                    "spanId":"0000000000000001",
                    "name":"chat gpt-5.4",
                    "kind":2,
                    "startTimeUnixNano":"1777396158011000000",
                    "endTimeUnixNano":"1777396160812706082",
                    "attributes":[
                        {"key":"gen_ai.conversation.id","value":{"stringValue":"otlp-conv-1"}},
                        {"key":"github.copilot.turn_id","value":{"stringValue":"0"}}
                    ]
                }]
            }]
        }]
    });
    let req = Request::builder().method("POST").uri("/v1/traces")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string())).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let raw: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM raw_records").fetch_one(&state.pool).await.unwrap();
    assert!(raw >= 1);
    let spans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spans").fetch_one(&state.pool).await.unwrap();
    assert_eq!(spans, 1);
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE conversation_id='otlp-conv-1'").fetch_one(&state.pool).await.unwrap();
    assert_eq!(sessions, 1);
}

// ===================== Live-visibility tests ============================
//
// These guard the bug we shipped: until tool spans are exposed by trace_id
// they are invisible during the live window between "tool finished" and
// "chat finished". The previous test-suite was a batch replay that always
// closed the chat span, so this case was masked.

const ORPHAN_TOOL_SPAN: &str = r#"{"type":"span","traceId":"aaaa","spanId":"tttt","parentSpanId":"chat-parent","name":"execute_tool bash","kind":0,"startTime":[1777396158,500000000],"endTime":[1777396158,600000000],"attributes":{"gen_ai.operation.name":"execute_tool","gen_ai.tool.name":"bash","gen_ai.tool.call.id":"call_xyz","gen_ai.tool.type":"function"},"events":[],"status":{"code":0}}"#;

const LATE_CHAT_PARENT: &str = r#"{"type":"span","traceId":"aaaa","spanId":"chat-parent","parentSpanId":null,"name":"chat gpt-5.4","kind":2,"startTime":[1777396158,11000000],"endTime":[1777396160,812706082],"attributes":{"gen_ai.operation.name":"chat","gen_ai.request.model":"gpt-5.4","gen_ai.conversation.id":"conv-live"},"events":[],"status":{"code":0}}"#;

#[tokio::test]
async fn tool_span_alone_is_visible_via_trace_endpoints() {
    // The whole point of the rewrite: a tool span arriving before its parent
    // chat span must be addressable by trace_id immediately. No session,
    // no conversation_id required.
    let state = fresh_state().await;
    ingest_lines(&state, &[ORPHAN_TOOL_SPAN.into()]).await;

    // spans table holds the real tool span + a placeholder for "chat-parent".
    let span_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spans WHERE trace_id='aaaa'")
        .fetch_one(&state.pool).await.unwrap();
    assert_eq!(span_count, 2);
    let ph_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM spans WHERE trace_id='aaaa' AND ingestion_state='placeholder'"
    ).fetch_one(&state.pool).await.unwrap();
    assert_eq!(ph_count, 1, "the unseen chat parent must be a placeholder");

    // The tool_calls projection row must already exist — clients filtering
    // by kind_class='execute_tool' have to find it without a chat ancestor.
    let tc: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tool_calls WHERE call_id='call_xyz'"
    ).fetch_one(&state.pool).await.unwrap();
    assert_eq!(tc, 1);

    // Sessions remain empty — conversation_id is unknown until chat lands.
    let sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
        .fetch_one(&state.pool).await.unwrap();
    assert_eq!(sessions, 0, "sessions must NOT be required for live visibility");
}

#[tokio::test]
async fn late_chat_upgrades_placeholder_in_place() {
    // After we add the chat span, the placeholder upgrades and the trace's
    // conversation_id becomes known — but no duplicate trace, no duplicate
    // tool call, no broken parent pointer.
    let state = fresh_state().await;
    ingest_lines(&state, &[ORPHAN_TOOL_SPAN.into()]).await;
    let pre_pks: Vec<i64> = sqlx::query_scalar("SELECT span_pk FROM spans ORDER BY span_pk")
        .fetch_all(&state.pool).await.unwrap();

    ingest_lines(&state, &[LATE_CHAT_PARENT.into()]).await;

    let post_pks: Vec<i64> = sqlx::query_scalar("SELECT span_pk FROM spans ORDER BY span_pk")
        .fetch_all(&state.pool).await.unwrap();
    assert_eq!(pre_pks, post_pks, "placeholder upgrade must keep the same span_pk");

    let ph_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM spans WHERE ingestion_state='placeholder'"
    ).fetch_one(&state.pool).await.unwrap();
    assert_eq!(ph_after, 0);

    let session_conv: Option<String> = sqlx::query_scalar(
        "SELECT conversation_id FROM sessions WHERE conversation_id='conv-live'"
    ).fetch_optional(&state.pool).await.unwrap();
    assert_eq!(session_conv.as_deref(), Some("conv-live"));

    // Tool call's conversation_id was filled retroactively (no need for the
    // frontend to refetch — projection rows are coherent).
    let tc_conv: Option<String> = sqlx::query_scalar(
        "SELECT conversation_id FROM tool_calls WHERE call_id='call_xyz'"
    ).fetch_one(&state.pool).await.unwrap();
    assert_eq!(tc_conv.as_deref(), Some("conv-live"));
}

#[tokio::test]
async fn trace_endpoint_query_works_without_session() {
    // The /api/traces query path itself: the SQL the endpoint runs must
    // return our orphan trace immediately. We don't spin up an HTTP client
    // here — we exercise the same SQL the handler runs.
    let state = fresh_state().await;
    ingest_lines(&state, &[ORPHAN_TOOL_SPAN.into()]).await;

    let row: Option<(String, i64, i64)> = sqlx::query_as(
        r#"SELECT s.trace_id, COUNT(*) AS span_count,
                  SUM(CASE WHEN s.ingestion_state='placeholder' THEN 1 ELSE 0 END) AS placeholder_count
             FROM spans s WHERE s.trace_id='aaaa' GROUP BY s.trace_id"#
    ).fetch_optional(&state.pool).await.unwrap();
    let (trace_id, span_count, ph) = row.expect("trace must be queryable");
    assert_eq!(trace_id, "aaaa");
    assert_eq!(span_count, 2);
    assert_eq!(ph, 1);
}
