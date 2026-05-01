//! Tests for ghcp_mon::normalize::handle_envelope. Many LLRs share this fixture.

use ghcp_mon::db;
use ghcp_mon::model::{
    Envelope, EventEnvelope, HrTime, LogEnvelope, MetricDataPoint, MetricEnvelope, SpanEnvelope,
    SpanStatus,
};
use ghcp_mon::normalize::{self, NormalizeCtx};
use ghcp_mon::ws::{Broadcaster, EventMsg};
use serde_json::{json, Map, Value};

async fn fresh_pool() -> sqlx::SqlitePool {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-norm-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    db::open(&dir.join("test.db")).await.unwrap()
}

async fn raw_id(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("INSERT INTO raw_records(source, record_type, body) VALUES('t','span','{}') RETURNING id")
        .fetch_one(pool).await.unwrap()
}

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

fn span(trace: &str, sid: &str, parent: Option<&str>, name: &str, attrs: Value) -> SpanEnvelope {
    SpanEnvelope {
        kind_tag: "span".into(),
        trace_id: trace.into(),
        span_id: sid.into(),
        parent_span_id: parent.map(String::from),
        name: name.into(),
        kind: None,
        start_time: HrTime::Nanos(1_000),
        end_time: Some(HrTime::Nanos(2_000)),
        attributes: obj(attrs),
        events: vec![],
        status: Some(SpanStatus { code: 0, message: None }),
        resource: None,
        instrumentation_scope: None,
    }
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<EventMsg>) -> Vec<EventMsg> {
    let mut v = Vec::new();
    while let Ok(m) = rx.try_recv() { v.push(m); }
    v
}

async fn handle(ctx: &NormalizeCtx<'_>, env: &Envelope) {
    normalize::handle_envelope(ctx, env).await.expect("handle ok");
}

// --- Span upsert / placeholder ---

#[tokio::test]
async fn span_upsert_keyed_by_trace_and_span_id_inserts_real_state() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let env = Envelope::Span(Box::new(span("t1","s1",None,"chat", json!({}))));
    handle(&ctx, &env).await;
    let (count, state): (i64, String) = sqlx::query_as(
        "SELECT COUNT(*), MAX(ingestion_state) FROM spans WHERE trace_id='t1' AND span_id='s1'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(state, "real");
}

#[tokio::test]
async fn span_upsert_on_conflict_forces_real_state_and_coalesces_optional_fields() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let mut s = span("tA","sA",None,"chat", json!({}));
    s.resource = Some(ghcp_mon::model::Resource {
        attributes: obj(json!({"service.name":"x"})),
        schema_url: None,
    });
    handle(&ctx, &Envelope::Span(Box::new(s.clone()))).await;
    // Re-ingest WITHOUT resource — should NOT blank existing resource_json.
    let mut s2 = s.clone();
    s2.resource = None;
    handle(&ctx, &Envelope::Span(Box::new(s2))).await;
    let (state, resource): (String, Option<String>) = sqlx::query_as(
        "SELECT ingestion_state, resource_json FROM spans WHERE trace_id='tA' AND span_id='sA'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(state, "real");
    assert!(resource.is_some(), "resource_json MUST be coalesced (preserved) on re-ingest");
}

#[tokio::test]
async fn placeholder_inserted_for_unseen_parent() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let child = span("tp","child", Some("missing-parent"), "chat", json!({}));
    handle(&ctx, &Envelope::Span(Box::new(child))).await;
    let (count, state): (i64, String) = sqlx::query_as(
        "SELECT COUNT(*), ingestion_state FROM spans WHERE trace_id='tp' AND span_id='missing-parent'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(state, "placeholder");
    let (name, attrs): (String, String) = sqlx::query_as(
        "SELECT name, attributes_json FROM spans WHERE span_id='missing-parent'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(name, "");
    assert_eq!(attrs, "{}");
}

#[tokio::test]
async fn placeholder_creation_is_idempotent_across_reingest() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let c1 = span("tx","c1", Some("p"), "chat", json!({}));
    let c2 = span("tx","c2", Some("p"), "chat", json!({}));
    handle(&ctx, &Envelope::Span(Box::new(c1))).await;
    handle(&ctx, &Envelope::Span(Box::new(c2))).await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM spans WHERE trace_id='tx' AND span_id='p'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1, "second placeholder insert MUST be a no-op");
}

#[tokio::test]
async fn placeholder_upgrade_flips_ingestion_state_to_real_with_upgrade_action() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(128);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    // Child first: creates placeholder for parent "P".
    handle(&ctx, &Envelope::Span(Box::new(span("tu","C",Some("P"),"chat",json!({}))))).await;
    let mut rx = bus.subscribe();
    // Now ingest the parent.
    handle(&ctx, &Envelope::Span(Box::new(span("tu","P",None,"chat",json!({}))))).await;
    let st: String = sqlx::query_scalar(
        "SELECT ingestion_state FROM spans WHERE trace_id='tu' AND span_id='P'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(st, "real");
    let evs = drain(&mut rx);
    let span_event = evs.iter().find(|m| m.kind == "span" && m.entity == "span"
        && m.payload.get("span_id").and_then(|v| v.as_str()) == Some("P"));
    let action = span_event.and_then(|m| m.payload.get("action").and_then(|v| v.as_str()));
    assert_eq!(action, Some("upgrade"), "span event MUST carry action=upgrade");
    let trace_event = evs.iter().find(|m| m.kind == "trace" && m.entity == "trace"
        && m.payload.get("span_id").and_then(|v| v.as_str()) == Some("P"));
    assert_eq!(trace_event.and_then(|m| m.payload.get("upgraded").and_then(|v| v.as_bool())), Some(true));
}

#[tokio::test]
async fn placeholder_upgrade_preserved_across_reingest_does_not_regress() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    handle(&ctx, &Envelope::Span(Box::new(span("tr","C",Some("P"),"chat",json!({}))))).await;
    handle(&ctx, &Envelope::Span(Box::new(span("tr","P",None,"chat",json!({}))))).await;
    handle(&ctx, &Envelope::Span(Box::new(span("tr","P",None,"chat",json!({}))))).await;
    let st: String = sqlx::query_scalar(
        "SELECT ingestion_state FROM spans WHERE trace_id='tr' AND span_id='P'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(st, "real");
}

// --- Span/trace events ---

#[tokio::test]
async fn span_normalize_emits_span_and_trace_events_with_action_insert() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(128);
    let mut rx = bus.subscribe();
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    handle(&ctx, &Envelope::Span(Box::new(span("ev","s",None,"chat",json!({}))))).await;
    let evs = drain(&mut rx);
    let span_ev = evs.iter().find(|m| m.kind == "span" && m.entity == "span"
        && m.payload.get("span_id").and_then(|v| v.as_str()) == Some("s")).expect("span event");
    assert_eq!(span_ev.payload["ingestion_state"], json!("real"));
    assert_eq!(span_ev.payload["trace_id"], json!("ev"));
    assert!(span_ev.payload.get("kind_class").is_some());
    assert_eq!(span_ev.payload["action"], json!("insert"));

    let trace_ev = evs.iter().find(|m| m.kind == "trace" && m.entity == "trace"
        && m.payload.get("span_id").and_then(|v| v.as_str()) == Some("s")).expect("trace event");
    assert_eq!(trace_ev.payload["ingestion_state"], json!("real"));
    assert_eq!(trace_ev.payload["upgraded"], json!(false));
}

#[tokio::test]
async fn placeholder_creation_emits_placeholder_events_only_when_inserted() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(128);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let mut rx = bus.subscribe();
    handle(&ctx, &Envelope::Span(Box::new(span("ph","c1",Some("p"),"chat",json!({}))))).await;
    let first = drain(&mut rx);
    assert!(first.iter().any(|m|
        m.kind=="span" && m.entity=="placeholder"
        && m.payload.get("span_id").and_then(|v| v.as_str()) == Some("p")
        && m.payload.get("action").and_then(|v| v.as_str()) == Some("insert")));
    assert!(first.iter().any(|m|
        m.kind=="trace" && m.entity=="trace"
        && m.payload.get("action").and_then(|v| v.as_str()) == Some("placeholder")
        && m.payload.get("ingestion_state").and_then(|v| v.as_str()) == Some("placeholder")));

    // Second child sharing the same parent must NOT re-emit placeholder events.
    let mut rx2 = bus.subscribe();
    handle(&ctx, &Envelope::Span(Box::new(span("ph","c2",Some("p"),"chat",json!({}))))).await;
    let second = drain(&mut rx2);
    assert!(!second.iter().any(|m|
        m.kind=="span" && m.entity=="placeholder"
        && m.payload.get("span_id").and_then(|v| v.as_str()) == Some("p")),
        "no placeholder event MUST fire for an existing placeholder row");
}

#[tokio::test]
async fn span_events_idempotently_replaced_on_reingest() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let mut s = span("se","s",None,"chat",json!({}));
    s.events = vec![
        EventEnvelope { name: "e1".into(), time: HrTime::Nanos(10), attributes: Map::new() },
        EventEnvelope { name: "e2".into(), time: HrTime::Nanos(20), attributes: Map::new() },
    ];
    handle(&ctx, &Envelope::Span(Box::new(s))).await;
    let n1: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM span_events WHERE span_pk = (SELECT span_pk FROM spans WHERE trace_id='se' AND span_id='s')"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(n1, 2);

    let mut s2 = span("se","s",None,"chat",json!({}));
    s2.events = vec![EventEnvelope { name: "only-this".into(), time: HrTime::Nanos(99), attributes: Map::new() }];
    handle(&ctx, &Envelope::Span(Box::new(s2))).await;
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM span_events WHERE span_pk = (SELECT span_pk FROM spans WHERE trace_id='se' AND span_id='s') ORDER BY event_pk"
    ).fetch_all(&pool).await.unwrap();
    assert_eq!(names, vec!["only-this"], "old span_events MUST be deleted before re-insert");
}

// --- Projections ---

#[tokio::test]
async fn invoke_agent_span_upserts_agent_run() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let attrs = json!({
        "gen_ai.agent.name": "Alpha",
        "gen_ai.agent.id": "agent-1",
        "gen_ai.agent.version": "v1",
        "gen_ai.conversation.id": "c-ag",
    });
    handle(&ctx, &Envelope::Span(Box::new(span("ta","sa",None,"invoke_agent",attrs)))).await;
    let (count, name, id, ver, cid): (i64, Option<String>, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as("SELECT COUNT(*), MAX(agent_name), MAX(agent_id), MAX(agent_version), MAX(conversation_id) FROM agent_runs")
            .fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(name.as_deref(), Some("Alpha"));
    assert_eq!(id.as_deref(), Some("agent-1"));
    assert_eq!(ver.as_deref(), Some("v1"));
    assert_eq!(cid.as_deref(), Some("c-ag"));
}

#[tokio::test]
async fn invoke_agent_falls_back_to_name_suffix_for_agent_name() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    handle(&ctx, &Envelope::Span(Box::new(
        span("ta2","sb",None,"invoke_agent ZetaBot", json!({"gen_ai.conversation.id":"c1"}))
    ))).await;
    let name: Option<String> = sqlx::query_scalar("SELECT agent_name FROM agent_runs LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(name.as_deref(), Some("ZetaBot"));
}

#[tokio::test]
async fn chat_span_upserts_chat_turn_with_token_counters() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let attrs = json!({
        "gen_ai.conversation.id": "cc",
        "github.copilot.interaction_id": "iid",
        "github.copilot.turn_id": "0",
        "gen_ai.request.model": "gpt-5",
        "gen_ai.usage.input_tokens": 10,
        "gen_ai.usage.output_tokens": 20,
        "gen_ai.usage.cache_read.input_tokens": 5,
        "gen_ai.usage.reasoning.output_tokens": 7,
    });
    handle(&ctx, &Envelope::Span(Box::new(span("tc","sc",None,"chat",attrs)))).await;
    let (cid, iid, tid, model, inp, out, cache, reason): (Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT conversation_id, interaction_id, turn_id, model, input_tokens, output_tokens, cache_read_tokens, reasoning_tokens FROM chat_turns")
            .fetch_one(&pool).await.unwrap();
    assert_eq!(cid.as_deref(), Some("cc"));
    assert_eq!(iid.as_deref(), Some("iid"));
    assert_eq!(tid.as_deref(), Some("0"));
    assert_eq!(model.as_deref(), Some("gpt-5"));
    assert_eq!(inp, Some(10));
    assert_eq!(out, Some(20));
    assert_eq!(cache, Some(5));
    assert_eq!(reason, Some(7));
}

#[tokio::test]
async fn chat_span_prefers_request_model_over_response_model() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let attrs = json!({
        "gen_ai.conversation.id":"cm",
        "gen_ai.request.model": "model-req",
        "gen_ai.response.model": "model-res",
    });
    handle(&ctx, &Envelope::Span(Box::new(span("tm","sm",None,"chat",attrs)))).await;
    let model: Option<String> = sqlx::query_scalar("SELECT model FROM chat_turns").fetch_one(&pool).await.unwrap();
    assert_eq!(model.as_deref(), Some("model-req"));
}

#[tokio::test]
async fn chat_token_usage_creates_chat_span_context_snapshot() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let attrs = json!({
        "gen_ai.conversation.id":"cs",
        "gen_ai.usage.input_tokens": 11,
        "gen_ai.usage.output_tokens": 22,
    });
    handle(&ctx, &Envelope::Span(Box::new(span("tcs","scs",None,"chat",attrs)))).await;
    let (count, source): (i64, Option<String>) = sqlx::query_as(
        "SELECT COUNT(*), MAX(source) FROM context_snapshots"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(source.as_deref(), Some("chat_span"));
    let (inp, out, cap): (Option<i64>, Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT input_tokens, output_tokens, captured_ns FROM context_snapshots")
            .fetch_one(&pool).await.unwrap();
    assert_eq!(inp, Some(11));
    assert_eq!(out, Some(22));
    assert_eq!(cap, Some(2_000), "captured_ns MUST be end_unix_ns when present");
}

#[tokio::test]
async fn execute_tool_span_upserts_tool_call() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let attrs = json!({
        "gen_ai.tool.call.id":"call-1",
        "gen_ai.tool.name":"bash",
        "gen_ai.tool.type":"function",
        "gen_ai.conversation.id":"ct",
    });
    handle(&ctx, &Envelope::Span(Box::new(span("tt","st",None,"execute_tool bash",attrs)))).await;
    let (cid, name, ty, conv): (Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT call_id, tool_name, tool_type, conversation_id FROM tool_calls"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(cid.as_deref(), Some("call-1"));
    assert_eq!(name.as_deref(), Some("bash"));
    assert_eq!(ty.as_deref(), Some("function"));
    assert_eq!(conv.as_deref(), Some("ct"));
}

#[tokio::test]
async fn external_tool_span_upserts_external_tool_call_with_fallback_attrs() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let attrs = json!({"gen_ai.tool.call.id":"X1","gen_ai.tool.name":"web"});
    handle(&ctx, &Envelope::Span(Box::new(span("tx","sx",None,"external_tool web",attrs)))).await;
    let (cid, name): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT call_id, tool_name FROM external_tool_calls").fetch_one(&pool).await.unwrap();
    assert_eq!(cid.as_deref(), Some("X1"), "MUST fall back to gen_ai.tool.call.id");
    assert_eq!(name.as_deref(), Some("web"));
}

#[tokio::test]
async fn external_tool_paired_to_internal_tool_call_by_call_id() {
    // Order 1: external first, then internal — internal MUST set paired_tool_call_pk on existing external row.
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let ext_attrs = json!({"github.copilot.external_tool.call_id":"K","github.copilot.external_tool.name":"x"});
    handle(&ctx, &Envelope::Span(Box::new(span("p","ext", None, "external_tool x", ext_attrs)))).await;
    let int_attrs = json!({"gen_ai.tool.call.id":"K","gen_ai.tool.name":"x"});
    handle(&ctx, &Envelope::Span(Box::new(span("p","int", None, "execute_tool x", int_attrs)))).await;
    let paired: Option<i64> = sqlx::query_scalar(
        "SELECT paired_tool_call_pk FROM external_tool_calls WHERE call_id='K'"
    ).fetch_one(&pool).await.unwrap();
    let internal_pk: Option<i64> = sqlx::query_scalar(
        "SELECT tool_call_pk FROM tool_calls WHERE call_id='K'"
    ).fetch_one(&pool).await.unwrap();
    assert!(paired.is_some());
    assert_eq!(paired, internal_pk);
}

#[tokio::test]
async fn projection_upserts_emit_derived_events() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(128);
    let mut rx = bus.subscribe();
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    handle(&ctx, &Envelope::Span(Box::new(span("d","a",None,"invoke_agent A",
        json!({"gen_ai.conversation.id":"c"}))))).await;
    handle(&ctx, &Envelope::Span(Box::new(span("d","b",Some("a"),"chat",
        json!({"gen_ai.conversation.id":"c"}))))).await;
    handle(&ctx, &Envelope::Span(Box::new(span("d","cc",Some("b"),"execute_tool x",
        json!({"gen_ai.conversation.id":"c","gen_ai.tool.call.id":"K","gen_ai.tool.name":"x"}))))).await;
    let evs = drain(&mut rx);
    assert!(evs.iter().any(|m| m.kind == "derived" && m.entity == "agent_run"),
        "MUST emit derived/agent_run");
    assert!(evs.iter().any(|m| m.kind == "derived" && m.entity == "chat_turn"),
        "MUST emit derived/chat_turn");
    assert!(evs.iter().any(|m| m.kind == "derived" && m.entity == "tool_call"),
        "MUST emit derived/tool_call");
}

// --- Sessions ---

#[tokio::test]
async fn session_upserted_per_conversation_id_with_min_max_timestamps() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let mut s1 = span("s1t","aa",None,"chat", json!({"gen_ai.conversation.id":"sess","gen_ai.request.model":"m1"}));
    s1.start_time = HrTime::Nanos(100);
    s1.end_time = Some(HrTime::Nanos(200));
    let mut s2 = span("s1t","bb",None,"chat", json!({"gen_ai.conversation.id":"sess"}));
    s2.start_time = HrTime::Nanos(50);
    s2.end_time = Some(HrTime::Nanos(75));
    handle(&ctx, &Envelope::Span(Box::new(s1))).await;
    handle(&ctx, &Envelope::Span(Box::new(s2))).await;
    let (first, last, model): (i64, i64, Option<String>) =
        sqlx::query_as("SELECT first_seen_ns, last_seen_ns, latest_model FROM sessions WHERE conversation_id='sess'")
            .fetch_one(&pool).await.unwrap();
    assert_eq!(first, 50, "first_seen_ns MUST be MIN of seen times");
    assert_eq!(last, 200, "last_seen_ns MUST be MAX of seen times");
    assert_eq!(model.as_deref(), Some("m1"));
}

#[tokio::test]
async fn session_counters_refreshed_on_session_upsert() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let cid = json!({"gen_ai.conversation.id":"sc"});
    handle(&ctx, &Envelope::Span(Box::new(span("s","root",None,"invoke_agent A", cid.clone())))).await;
    handle(&ctx, &Envelope::Span(Box::new(span("s","chat1",Some("root"),"chat", cid.clone())))).await;
    let attrs_tool = json!({"gen_ai.conversation.id":"sc","gen_ai.tool.call.id":"k1","gen_ai.tool.name":"t"});
    handle(&ctx, &Envelope::Span(Box::new(span("s","tool1",Some("chat1"),"execute_tool t", attrs_tool)))).await;
    let (chats, tools, agents): (i64, i64, i64) = sqlx::query_as(
        "SELECT chat_turn_count, tool_call_count, agent_run_count FROM sessions WHERE conversation_id='sc'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(chats, 1);
    assert_eq!(tools, 1);
    assert_eq!(agents, 1);
}

#[tokio::test]
async fn session_upsert_emits_derived_session_update_event() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(128);
    let mut rx = bus.subscribe();
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    handle(&ctx, &Envelope::Span(Box::new(span("ses","x",None,"chat",
        json!({"gen_ai.conversation.id":"cid-q","gen_ai.request.model":"m"}))))).await;
    let evs = drain(&mut rx);
    let ev = evs.iter().find(|m| m.kind == "derived" && m.entity == "session").expect("derived/session");
    assert_eq!(ev.payload["action"], json!("update"));
    assert_eq!(ev.payload["conversation_id"], json!("cid-q"));
    assert_eq!(ev.payload["latest_model"], json!("m"));
}

// --- Conversation inheritance & projection pointers ---

#[tokio::test]
async fn effective_conversation_id_inherited_from_ancestors() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    handle(&ctx, &Envelope::Span(Box::new(span("eff","root",None,"invoke_agent A",
        json!({"gen_ai.conversation.id":"inherited"}))))).await;
    handle(&ctx, &Envelope::Span(Box::new(span("eff","child",Some("root"),"chat",
        json!({}))))).await;  // no cid
    // Child's chat_turns row MUST inherit conversation_id.
    let cid: Option<String> = sqlx::query_scalar(
        "SELECT conversation_id FROM chat_turns WHERE span_pk = (SELECT span_pk FROM spans WHERE span_id='child')"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(cid.as_deref(), Some("inherited"));
}

#[tokio::test]
async fn projection_pointers_resolved_via_ancestor_walk() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let cid = json!({"gen_ai.conversation.id":"pp"});
    handle(&ctx, &Envelope::Span(Box::new(span("pp","ag",None,"invoke_agent A", cid.clone())))).await;
    handle(&ctx, &Envelope::Span(Box::new(span("pp","chat",Some("ag"),"chat", cid.clone())))).await;
    let attrs_tool = json!({"gen_ai.conversation.id":"pp","gen_ai.tool.call.id":"k","gen_ai.tool.name":"t"});
    handle(&ctx, &Envelope::Span(Box::new(span("pp","tool",Some("chat"),"execute_tool t", attrs_tool)))).await;
    let (ar, ct): (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT agent_run_pk, chat_turn_pk FROM tool_calls WHERE call_id='k'"
    ).fetch_one(&pool).await.unwrap();
    assert!(ar.is_some(), "tool_calls.agent_run_pk MUST be set via ancestor walk");
    assert!(ct.is_some(), "tool_calls.chat_turn_pk MUST be set via ancestor walk");
}

#[tokio::test]
async fn forward_resolve_descendants_on_parent_arrival() {
    // Child arrives first (no parent yet); parent arrives later — child's projection pointers must back-fill.
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let attrs_tool = json!({"gen_ai.tool.call.id":"k","gen_ai.tool.name":"t"});
    handle(&ctx, &Envelope::Span(Box::new(span("fr","tool",Some("chat"),"execute_tool t", attrs_tool)))).await;
    // Initially: no chat_turn_pk on tool_call row.
    let pre: Option<i64> = sqlx::query_scalar(
        "SELECT chat_turn_pk FROM tool_calls WHERE call_id='k'"
    ).fetch_one(&pool).await.unwrap();
    assert!(pre.is_none(), "child arriving alone has no chat_turn_pk yet");

    // Now ingest the parent chat span.
    handle(&ctx, &Envelope::Span(Box::new(span("fr","chat",None,"chat",
        json!({"gen_ai.conversation.id":"frc"}))))).await;

    let post: Option<i64> = sqlx::query_scalar(
        "SELECT chat_turn_pk FROM tool_calls WHERE call_id='k'"
    ).fetch_one(&pool).await.unwrap();
    assert!(post.is_some(),
        "child's chat_turn_pk MUST be back-filled on parent arrival");
    let cid: Option<String> = sqlx::query_scalar(
        "SELECT conversation_id FROM tool_calls WHERE call_id='k'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(cid.as_deref(), Some("frc"),
        "child's conversation_id MUST inherit from late-arriving parent");
}

// --- Span-event derivations ---

#[tokio::test]
async fn hook_start_event_derives_hook_invocation() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let mut s = span("hk","s",None,"chat", json!({"gen_ai.conversation.id":"ch"}));
    s.events = vec![EventEnvelope {
        name: "github.copilot.hook.start".into(),
        time: HrTime::Nanos(1234),
        attributes: obj(json!({
            "github.copilot.hook.invocation_id":"inv-1",
            "github.copilot.hook.type":"pre",
        })),
    }];
    handle(&ctx, &Envelope::Span(Box::new(s))).await;
    let (count, ty, start): (i64, Option<String>, Option<i64>) = sqlx::query_as(
        "SELECT COUNT(*), MAX(hook_type), MAX(start_unix_ns) FROM hook_invocations WHERE invocation_id='inv-1'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(ty.as_deref(), Some("pre"));
    assert_eq!(start, Some(1234));
}

#[tokio::test]
async fn hook_end_event_completes_hook_invocation_with_duration() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    // start
    let mut s = span("hk","s1",None,"chat", json!({}));
    s.events = vec![EventEnvelope {
        name: "github.copilot.hook.start".into(),
        time: HrTime::Nanos(100),
        attributes: obj(json!({"github.copilot.hook.invocation_id":"inv-2","github.copilot.hook.type":"pre"})),
    }];
    handle(&ctx, &Envelope::Span(Box::new(s))).await;
    // end (different span)
    let mut s2 = span("hk","s2",None,"chat", json!({}));
    s2.events = vec![EventEnvelope {
        name: "github.copilot.hook.end".into(),
        time: HrTime::Nanos(500),
        attributes: obj(json!({"github.copilot.hook.invocation_id":"inv-2"})),
    }];
    handle(&ctx, &Envelope::Span(Box::new(s2))).await;
    let (end, dur): (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT end_unix_ns, duration_ns FROM hook_invocations WHERE invocation_id='inv-2'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(end, Some(500));
    assert_eq!(dur, Some(400), "duration_ns MUST be end - start when start is set");
}

#[tokio::test]
async fn skill_invoked_event_records_skill_invocation_idempotently() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let mut s = span("sk","s",None,"chat", json!({"gen_ai.conversation.id":"ck"}));
    let ev = EventEnvelope {
        name: "github.copilot.skill.invoked".into(),
        time: HrTime::Nanos(7),
        // Cheatsheet does not name the attribute keys; try several plausible spellings.
        attributes: obj(json!({
            "skill_name":"lint","skill_path":"/x",
            "github.copilot.skill.name":"lint","github.copilot.skill.path":"/x",
            "skill.name":"lint","skill.path":"/x",
            "name":"lint","path":"/x",
        })),
    };
    s.events = vec![ev.clone(), ev.clone()];
    handle(&ctx, &Envelope::Span(Box::new(s.clone()))).await;
    // Re-ingest.
    handle(&ctx, &Envelope::Span(Box::new(s))).await;
    // Idempotency: regardless of which attribute spelling the implementation reads,
    // re-ingestion at the same (span_pk, invoked_unix_ns, skill_name) MUST NOT duplicate.
    let by_name: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(c),0) FROM (SELECT COUNT(*) AS c FROM skill_invocations GROUP BY span_pk, invoked_unix_ns, skill_name)"
    ).fetch_one(&pool).await.unwrap();
    assert!(by_name <= 1, "skill_invocations MUST be idempotent on (span_pk, invoked_unix_ns, skill_name); got max group size {}", by_name);
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM skill_invocations WHERE invoked_unix_ns=7"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(total, 1, "exactly one skill_invocation row MUST be present after re-ingest of identical events");
}

#[tokio::test]
async fn usage_info_event_creates_context_snapshot_with_event_source() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let mut s = span("ui","s",None,"chat", json!({"gen_ai.conversation.id":"cu"}));
    s.events = vec![EventEnvelope {
        name: "github.copilot.session.usage_info".into(),
        time: HrTime::Nanos(99),
        attributes: obj(json!({
            "github.copilot.token_limit": 1000,
            "github.copilot.current_tokens": 250,
            "github.copilot.messages_length": 8,
        })),
    }];
    handle(&ctx, &Envelope::Span(Box::new(s))).await;
    let (count, source, lim, cur, ml): (i64, Option<String>, Option<i64>, Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT COUNT(*), MAX(source), MAX(token_limit), MAX(current_tokens), MAX(messages_length) FROM context_snapshots WHERE source='usage_info_event'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(source.as_deref(), Some("usage_info_event"));
    assert_eq!(lim, Some(1000));
    assert_eq!(cur, Some(250));
    assert_eq!(ml, Some(8));
}

// --- Chat-turn tool count refresh ---

#[tokio::test]
async fn chat_turn_tool_count_refreshed_only_for_internal_tool_calls() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let cid_attrs = json!({"gen_ai.conversation.id":"tcc"});
    handle(&ctx, &Envelope::Span(Box::new(span("ttc","chat",None,"chat", cid_attrs.clone())))).await;
    let int_attrs = json!({"gen_ai.conversation.id":"tcc","gen_ai.tool.call.id":"i1","gen_ai.tool.name":"x"});
    handle(&ctx, &Envelope::Span(Box::new(span("ttc","tool",Some("chat"),"execute_tool x", int_attrs)))).await;
    let ext_attrs = json!({"gen_ai.conversation.id":"tcc","gen_ai.tool.call.id":"e1","gen_ai.tool.name":"y"});
    handle(&ctx, &Envelope::Span(Box::new(span("ttc","ext",Some("chat"),"external_tool y", ext_attrs)))).await;
    let count: i64 = sqlx::query_scalar(
        "SELECT tool_call_count FROM chat_turns WHERE span_pk = (SELECT span_pk FROM spans WHERE trace_id='ttc' AND span_id='chat')"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1, "external_tool_calls MUST NOT be counted; only tool_calls");
}

// --- Metric path ---

#[tokio::test]
async fn metric_data_points_persisted_to_metric_points_with_event_emission() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let mut rx = bus.subscribe();
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };

    let env = Envelope::Metric(Box::new(MetricEnvelope {
        kind_tag: "metric".into(),
        name: "my.metric".into(),
        description: Some("desc".into()),
        unit: Some("ms".into()),
        data_points: vec![
            MetricDataPoint {
                attributes: Map::new(),
                start_time: Some(HrTime::Nanos(10)),
                end_time: Some(HrTime::Nanos(20)),
                value: json!({"asDouble": 1.0}),
            },
            MetricDataPoint {
                attributes: Map::new(),
                start_time: None,
                end_time: None,
                value: json!({"asInt": 7}),
            },
        ],
        resource: None,
        instrumentation_scope: None,
    }));
    handle(&ctx, &env).await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM metric_points WHERE metric_name='my.metric'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 2);
    let evs = drain(&mut rx);
    let metric_ev = evs.iter().find(|m| m.kind == "metric" && m.entity == "metric").expect("metric event");
    assert_eq!(metric_ev.payload["name"], json!("my.metric"));
    assert_eq!(metric_ev.payload["points"], json!(2));
}

#[tokio::test]
async fn logs_envelope_is_no_op() {
    let pool = fresh_pool().await;
    let bus = Broadcaster::new(64);
    let mut rx = bus.subscribe();
    let rid = raw_id(&pool).await;
    let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: rid };
    let env = Envelope::Log(Box::new(LogEnvelope {
        kind_tag: "log".into(),
        body: json!("hi"),
        attributes: Map::new(),
        time_unix_nano: None,
        resource: None,
        instrumentation_scope: None,
        severity_text: None,
    }));
    handle(&ctx, &env).await;
    let mp: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM metric_points").fetch_one(&pool).await.unwrap();
    let sp: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spans").fetch_one(&pool).await.unwrap();
    assert_eq!(mp, 0);
    assert_eq!(sp, 0);
    assert!(drain(&mut rx).is_empty(), "Log envelope MUST NOT emit any bus events");
}
