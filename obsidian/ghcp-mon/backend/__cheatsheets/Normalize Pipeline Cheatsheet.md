---
type: cheatsheet
---
Source: `src/normalize/mod.rs`. Crate path: `ghcp_mon::normalize`.

The biggest module: takes `Envelope`s and projects them into the normalized SQLite tables, emitting events on the broadcast bus. Depends on [[Model Envelope Cheatsheet]], [[Broadcaster Cheatsheet]], [[DB Module Cheatsheet]], [[AppError Cheatsheet]] (indirectly via callers).

## Extract — public surface

```rust
use crate::model::*;
use crate::ws::{Broadcaster, EventMsg};
use sqlx::SqlitePool;

pub struct NormalizeCtx<'a> {
    pub pool: &'a SqlitePool,
    pub bus: &'a Broadcaster,
    pub raw_record_id: i64,
}

pub async fn handle_envelope(
    ctx: &NormalizeCtx<'_>,
    env: &Envelope,
) -> anyhow::Result<()>;
```

That is the **only** public function in this module — everything else (`normalize_span`, `normalize_metric`, `ensure_placeholder`, `upsert_agent_run`, `upsert_chat_turn`, `upsert_tool_call`, `upsert_external_tool_call`, `walk_ancestors`, `resolve_projection_pointers`, `forward_resolve_descendants`, `effective_conversation_id`, `upsert_session_for_span`, `derive_from_events`, `insert_chat_context_snapshot`, `refresh_chat_turn_tool_count_for_span`, `emit_projection_event`) is private. Tests must observe behavior **only** via:

1. The `NormalizeCtx` (constructed with a real pool and broadcaster).
2. The resulting database state (queryable with `sqlx::query_as`).
3. Events produced on the broadcaster (`bus.subscribe()` before calling).

`handle_envelope` dispatch:
- `Envelope::Span(_)` → span normalization path.
- `Envelope::Metric(_)` → metric normalization path.
- `Envelope::Log(_)` → no-op (`Ok(())`).

## Extract — observable database surface

Tables touched (exact names, all created by migrations):
- `spans (span_pk PK, trace_id, span_id, parent_span_id, name, kind, start_unix_ns, end_unix_ns, duration_ns, status_code, status_message, attributes_json, resource_json, scope_name, scope_version, ingestion_state, first_seen_raw_id, last_seen_raw_id)` with UNIQUE `(trace_id, span_id)`.
- `span_events (event_pk PK, span_pk FK, raw_record_id, name, time_unix_ns, attributes_json)`.
- `agent_runs (agent_run_pk PK, span_pk FK UNIQUE, conversation_id, agent_id, agent_name, agent_version, start_unix_ns, end_unix_ns, duration_ns, parent_agent_run_pk, parent_span_pk)`.
- `chat_turns (turn_pk PK, span_pk FK UNIQUE, conversation_id, interaction_id, turn_id, model, input_tokens, output_tokens, cache_read_tokens, reasoning_tokens, tool_call_count, agent_run_pk, start_unix_ns, end_unix_ns)`.
- `tool_calls (tool_call_pk PK, span_pk FK UNIQUE, call_id, tool_name, tool_type, conversation_id, start_unix_ns, end_unix_ns, duration_ns, status_code, agent_run_pk, chat_turn_pk)`.
- `external_tool_calls (ext_pk PK, span_pk FK UNIQUE, call_id, tool_name, paired_tool_call_pk, conversation_id, start_unix_ns, end_unix_ns, duration_ns, agent_run_pk, chat_turn_pk)`.
- `sessions (conversation_id PK, first_seen_ns, last_seen_ns, latest_model, chat_turn_count, tool_call_count, agent_run_count)`.
- `hook_invocations (invocation_id UNIQUE, hook_type, span_pk, conversation_id, start_unix_ns, end_unix_ns, duration_ns, agent_run_pk, chat_turn_pk, tool_call_pk)`.
- `skill_invocations (PK, span_pk, skill_name, skill_path, invoked_unix_ns, conversation_id, agent_run_pk, chat_turn_pk)` with UNIQUE `(span_pk, invoked_unix_ns, skill_name)`.
- `context_snapshots (ctx_pk PK, span_pk, conversation_id, chat_turn_pk, captured_ns, token_limit, current_tokens, messages_length, input_tokens, output_tokens, cache_read_tokens, reasoning_tokens, source)` with UNIQUE `(span_pk, source, captured_ns)`. `source` ∈ {`"chat_span"`, `"usage_info_event"`}.
- `metric_points (raw_record_id, metric_name, description, unit, start_unix_ns, end_unix_ns, attributes_json, value_json, resource_json, scope_name, scope_version)`.

`spans.ingestion_state` ∈ `{ "real", "placeholder" }`.

## Extract — observable event surface

Each `EventMsg` is sent on `ctx.bus`. Pattern: `{kind, entity, payload}`. The set of messages emitted per `handle_envelope`:

- Real span insert/upgrade: two messages — `kind="span" entity="span"` (with `action ∈ {"insert","upgrade"}`, `trace_id`, `span_id`, `parent_span_id`, `name`, `kind_class`, `ingestion_state="real"`, `span_pk`) and `kind="trace" entity="trace"` (with `action`, `trace_id`, `span_id`, `ingestion_state`, `upgraded: bool`).
- Placeholder creation (a parent isn't yet known): `kind="span" entity="placeholder"` (`action="insert"`, `trace_id`, `span_id`, `span_pk`) and `kind="trace" entity="trace"` (`action="placeholder"`, `ingestion_state="placeholder"`, `upgraded=false`).
- Projection upserts after a real span: at most one of `kind="derived" entity="agent_run" | "chat_turn" | "tool_call"` — `external_tool` and `other` kind classes do not emit a derived event.
- Session derivation: `kind="derived" entity="session"` (`action="update"`, `conversation_id`, `latest_model`).
- Metric ingest: `kind="metric" entity="metric"` produced via `EventMsg::raw("metric", json!({"name": <name>, "points": <count>}))`.

Span-class dispatch (after `normalize_span` writes the row) follows `SpanKindClass::from_name(&s.name)`:
- `InvokeAgent` → `upsert_agent_run` with attrs `gen_ai.agent.{name,id,version}` and `gen_ai.conversation.id`. `agent_name` falls back to `s.name.strip_prefix("invoke_agent ")`.
- `Chat` → `upsert_chat_turn` reading `gen_ai.conversation.id`, `github.copilot.interaction_id`, `github.copilot.turn_id`, `gen_ai.request.model` (or `gen_ai.response.model`), and the `gen_ai.usage.*_tokens` set. If any usage attribute is non-null, additionally inserts a `context_snapshots` row with `source="chat_span"`.
- `ExecuteTool` → `upsert_tool_call` reading `gen_ai.tool.{call.id,name,type}` and `gen_ai.conversation.id`. After insert, pairs any prior `external_tool_calls` row with the same `call_id` (sets `paired_tool_call_pk`).
- `ExternalTool` → `upsert_external_tool_call` reading `github.copilot.external_tool.{call_id,name}` (with fallback to `gen_ai.tool.{call.id,name}`) and pairing forward to an existing `tool_calls.tool_call_pk` if any exists for that `call_id`.
- `Other` → no projection.

Span event handlers (`derive_from_events`):
- `"github.copilot.hook.start"` → upsert `hook_invocations` row keyed by `github.copilot.hook.invocation_id`, set `hook_type` from `github.copilot.hook.type`, store `start_unix_ns = ev.time.to_unix_nanos()`.
- `"github.copilot.hook.end"` → upsert by `invocation_id`, set `end_unix_ns`, compute `duration_ns = end - start` if `start_unix_ns IS NOT NULL`.
- `"github.copilot.skill.invoked"` → insert `skill_invocations` (idempotent on `(span_pk, invoked_unix_ns, skill_name)`).
- `"github.copilot.session.usage_info"` → upsert `context_snapshots` row keyed by `(span_pk, source, captured_ns)` with `source = "usage_info_event"`, fields `token_limit`, `current_tokens`, `messages_length` from event attributes `github.copilot.{token_limit,current_tokens,messages_length}`.

Projection-pointer resolution (post-insert):
- `walk_ancestors` does a recursive trace-scoped walk to depth 64. It surfaces the **nearest non-self** `agent_run_pk`, `chat_turn_pk`, `tool_call_pk`, the `nearest_invoker_span_pk` (any ancestor that has a `chat_turns` or `tool_calls` row), and the nearest `gen_ai.conversation.id` from any ancestor's attributes.
- These are written into `agent_runs`, `chat_turns`, `tool_calls`, `external_tool_calls`, `hook_invocations`, `skill_invocations`, `context_snapshots` with `COALESCE(existing, new)` — i.e. resolved values are **only set if currently NULL** (idempotent).
- After projecting the current span, the function recursively re-resolves all descendants in the same trace (depth ≤ 128), so a late-arriving ancestor "fills in" pointers for already-seen children.

Session row derivation:
- Whenever a real span has an effective conversation id (its own attribute or an ancestor's), `upsert_session_for_span` upserts a `sessions` row: `first_seen_ns = MIN(...)`, `last_seen_ns = MAX(...)`, `latest_model = COALESCE(new, existing)`. Then it refreshes `chat_turn_count`, `tool_call_count`, `agent_run_count` by re-counting the projection tables for that `conversation_id`.

Chat-turn tool count refresh:
- `refresh_chat_turn_tool_count_for_span` runs at the end of every span normalization: collects every distinct `chat_turn` reachable from this span (the chat itself, or via `tool_calls.chat_turn_pk` / `external_tool_calls.chat_turn_pk` whose `span_pk = ?`), and updates each `chat_turns.tool_call_count` to `(SELECT COUNT(*) FROM tool_calls WHERE chat_turn_pk = ?1)`. (External tool calls are **not** counted here — only `tool_calls`.)

Metric path (`normalize_metric`):
- Inserts one `metric_points` row per `MetricDataPoint`. `attributes_json`, `value_json`, `resource_json` are the result of `serde_json::to_string` on the corresponding fields.
- `start_unix_ns` / `end_unix_ns` come from `dp.start_time.to_unix_nanos()` / `dp.end_time.to_unix_nanos()` (each `Option<HrTime>`).
- After all data points are inserted, emits `EventMsg::raw("metric", json!({"name": m.name, "points": m.data_points.len()}))`.
- Logs are not normalized: `Envelope::Log(_)` is a no-op.

Re-ingestion (placeholder upgrade):
- The `spans` upsert is keyed by `(trace_id, span_id)`. On conflict, columns are updated; `ingestion_state` is forced to `'real'`. Span events are **idempotently** replaced via `DELETE FROM span_events WHERE span_pk = ?` before re-inserting.
- Whether the prior row was a placeholder is tracked (`was_placeholder`) and influences the emitted `action` ∈ {`"insert"`, `"upgrade"`} on the bus events.

## Suggested Test Strategy

This is integration-style territory. Do not try to mock `SqlitePool` or `Broadcaster` — both work cheaply with real instances.

Setup pattern:

```rust
async fn make_ctx() -> (sqlx::SqlitePool, Broadcaster, /* tempfile guard */) {
    let path = unique_tempfile_path();
    let pool = ghcp_mon::db::open(&path).await.unwrap();
    let bus = Broadcaster::new(256);
    (pool, bus, path /* clean up at drop */)
}

let mut rx = bus.subscribe();
let ctx = NormalizeCtx { pool: &pool, bus: &bus, raw_record_id: /* insert one raw row first */ };
ghcp_mon::normalize::handle_envelope(&ctx, &env).await.unwrap();
// then drain rx for emitted EventMsgs and inspect tables via sqlx::query_as
```

Tactics:
- Build envelopes by hand (`SpanEnvelope { ... }`); use `Default` impls and override only the fields you need. `attributes` is a `serde_json::Map<String, Value>` — use `serde_json::json!({ ... }).as_object().unwrap().clone()` to construct.
- For each LLR, drive `handle_envelope` once (or twice for re-ingest / parent-arrival cases) and assert table state with explicit SQL like `SELECT count(*), span_pk, trace_id, ingestion_state FROM spans WHERE ...`.
- For event assertions, drain the broadcaster with `tokio::time::timeout` and `rx.try_recv()` in a loop, collecting every message into a `Vec<EventMsg>`; then filter by `(kind, entity)` and assert both presence and `payload` field values via `serde_json::Value` equality.
- Use `assert_matches!` (not in deps) sparingly — `match`/`if let` is fine.
- Tests that require a known clock can pass `HrTime::Nanos(...)` literally. The `to_unix_nanos()` saturating-arithmetic conversion is deterministic.
- For "Forward resolve descendants on parent arrival": ingest a child first (its parent will create a placeholder), then ingest the parent. Verify the child's `agent_runs.parent_span_pk` etc. are now populated, NOT just the parent's.
- For "Effective conversation id inherited from ancestors": ingest a chain (root with `gen_ai.conversation.id` set, child with no such attribute) and assert that the child's projection rows carry the inherited `conversation_id`.
- For "Placeholder upgrade preserved across reingest": ingest a child (creates placeholder for the parent), then ingest the placeholder's parent twice; the second pass should NOT regress `ingestion_state` back to `"placeholder"` (the upsert hardcodes `ingestion_state = 'real'`).
- For "Span events idempotently replaced on span upsert": ingest a span with two events, then re-ingest the same span with one different event. Final `span_events` row count for that `span_pk` should be 1 and contain only the new event.
- For "External tool paired to internal tool call by call id": ingest the external tool span first (no pairing yet), then the internal `execute_tool` span sharing the same `gen_ai.tool.call.id` / `github.copilot.external_tool.call_id`. Assert `external_tool_calls.paired_tool_call_pk` becomes the new `tool_calls.tool_call_pk`. Reverse order should also pair (the internal-first path is handled inside `upsert_external_tool_call`).
- For session-counter and chat-turn-tool-count tests, ingest the relevant chat/tool spans then `SELECT chat_turn_count, tool_call_count, agent_run_count FROM sessions WHERE conversation_id = ?` (and the `chat_turns.tool_call_count` field). Verify only `tool_calls` are counted in the chat-turn tool count, **not** `external_tool_calls`.
- For "Metric ingest emits raw metric event": send a `MetricEnvelope` with two data points; expect exactly two `metric_points` rows and one `EventMsg { kind: "metric", entity: "metric", payload: { name, points: 2 } }` on the bus.
- For "Logs not normalized currently": send `Envelope::Log(...)`; assert no rows are written to `metric_points`/`spans` and no events are sent.

Insert a `raw_records` row (e.g. via [[DAO Cheatsheet]]'s `insert_raw`) before constructing each `NormalizeCtx` so that the FK `raw_records(id)` is valid; otherwise foreign keys (turned on by `db::open`) reject the insert.
