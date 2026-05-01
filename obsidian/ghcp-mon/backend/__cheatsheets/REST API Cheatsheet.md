---
type: cheatsheet
---
Source: `src/api/mod.rs`. Crate path: `ghcp_mon::api`.

REST API handlers. All take `State<AppState>` and return `AppResult<Json<Value>>`. Depends on [[Server Router Cheatsheet]] (`AppState`, `api_router`), [[Local Session Cheatsheet]] (`resolve_session_state_dir`, `read_workspace_yaml`), [[AppError Cheatsheet]], [[Model Envelope Cheatsheet]] (`SpanKindClass`), [[Broadcaster Cheatsheet]] (the `bus.send` for delete events).

## Extract — handler signatures

```rust
use axum::{extract::{Path, Query, State}, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::Value;
use crate::server::AppState;
use crate::error::{AppError, AppResult};

pub async fn healthz() -> impl IntoResponse;

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub since: Option<i64>,
    pub session: Option<String>,
    pub kind: Option<String>,    // 'invoke_agent' | 'chat' | 'execute_tool' | 'external_tool' | 'other'
    #[serde(rename = "type")]
    pub type_filter: Option<String>,
}

pub async fn list_sessions(State(s): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Json<Value>>;
pub async fn get_session(State(s): State<AppState>, Path(cid): Path<String>) -> AppResult<Json<Value>>;
pub async fn delete_session(State(s): State<AppState>, Path(cid): Path<String>) -> AppResult<Json<Value>>;
pub async fn get_session_span_tree(State(s): State<AppState>, Path(cid): Path<String>) -> AppResult<Json<Value>>;
pub async fn list_session_contexts(State(s): State<AppState>, Path(cid): Path<String>) -> AppResult<Json<Value>>;
pub async fn list_spans(State(s): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Json<Value>>;
pub async fn get_span(State(s): State<AppState>, Path((trace_id, span_id)): Path<(String, String)>) -> AppResult<Json<Value>>;
pub async fn list_traces(State(s): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Json<Value>>;
pub async fn get_trace(State(s): State<AppState>, Path(trace_id): Path<String>) -> AppResult<Json<Value>>;
pub async fn list_raw(State(s): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Json<Value>>;
```

Routes (mounted by [[Server Router Cheatsheet]]):
- `GET  /api/healthz`
- `GET  /api/sessions`
- `GET, DELETE /api/sessions/:cid`
- `GET  /api/sessions/:cid/span-tree`
- `GET  /api/sessions/:cid/contexts`
- `GET  /api/spans`
- `GET  /api/spans/:trace_id/:span_id`
- `GET  /api/traces`
- `GET  /api/traces/:trace_id`
- `GET  /api/raw`

## Extract — observable response shapes

(All responses are `application/json`. Any handler may return an `AppError::NotFound` (404) or other `AppError` per [[AppError Cheatsheet]].)

`healthz`: `{"ok": true}`.

`list_sessions`:
```
{"sessions": [
  {"conversation_id": str, "first_seen_ns": i64?, "last_seen_ns": i64?,
   "latest_model": str?, "chat_turn_count": i64,
   "tool_call_count": i64, "agent_run_count": i64,
   "local_name": str?, "user_named": bool?, "cwd": str?, "branch": str?},
  ...
]}
```
- Ordered by `COALESCE(last_seen_ns, 0) DESC`.
- Filtered by `last_seen_ns >= since` (default `since=0`).
- Limit clamped via `clamp(1, 500)` from a default of `50`.
- Local fields (`local_name`, `user_named`, `cwd`, `branch`) come from `local_session::read_workspace_yaml(<resolved base>, &cid)`; missing/parse-fail → all four `None`.

`get_session`: same fields as above plus `"span_count": i64` (count of `spans` rows whose `attributes_json -> gen_ai.conversation.id` equals `cid`). 404 when no session row exists.

`delete_session`: 404 when missing. Otherwise:
- Computes the set of `trace_id`s involved via a UNION across `spans` (by `gen_ai.conversation.id`), `agent_runs`, `chat_turns`, `tool_calls`.
- In one transaction: `DELETE FROM spans WHERE trace_id IN (...)` (cascades to `span_events` and projection rows tagged by FK), then explicit deletes of `context_snapshots`, `hook_invocations`, `skill_invocations`, `external_tool_calls`, `tool_calls`, `chat_turns`, `agent_runs`, `sessions` rows tagged with this `conversation_id`.
- On success: `bus.send(EventMsg { kind:"derived", entity:"session", payload: {"action":"delete","conversation_id":cid}})`.
- Response body: `{"deleted": true, "conversation_id": cid, "trace_count": <usize>}`.

`get_session_span_tree`:
```
{"conversation_id": cid, "tree": [<node>, ...]}
```
Node:
```
{"span_pk": i64, "trace_id": str, "span_id": str, "parent_span_id": str?,
 "name": str, "kind_class": "invoke_agent"|"chat"|"execute_tool"|"external_tool"|"other",
 "ingestion_state": "real"|"placeholder",
 "start_unix_ns": i64?, "end_unix_ns": i64?,
 "projection": { "chat_turn"?: {...}, "tool_call"?: {...}, "agent_run"?: {...}, "external_tool_call"?: {...} },
 "children": [<node>, ...]}
```
- Spans included = trace-scoped union: every `trace_id` reachable from the conversation seed (spans tagged by attribute, plus `agent_runs/chat_turns/tool_calls` whose `conversation_id = cid`), then all spans sharing those `trace_id`s.
- Children sorted newest-first by `start_unix_ns`; null-start (placeholder) entries float to the top of the children list and roots list.
- Roots = nodes whose `parent_span_id` is null OR whose parent isn't in the result set.

`list_session_contexts`:
```
{"conversation_id": cid, "context_snapshots": [
  {"ctx_pk": i64, "span_pk": i64?, "captured_ns": i64,
   "token_limit": i64?, "current_tokens": i64?, "messages_length": i64?,
   "input_tokens": i64?, "output_tokens": i64?,
   "cache_read_tokens": i64?, "reasoning_tokens": i64?,
   "source": str?},  // "chat_span" | "usage_info_event"
  ...
]}
```
Ordered by `captured_ns ASC`.

`list_spans`:
```
{"spans": [
  {"span_pk": i64, "trace_id": str, "span_id": str, "parent_span_id": str?,
   "name": str, "kind_class": <classified>,
   "start_unix_ns": i64?, "end_unix_ns": i64?,
   "ingestion_state": str},
  ...
]}
```
- Filters: `?session=<cid>` (matches the same UNION as `delete_session`), `?kind=<class>` (matches the SQL CASE that mirrors `SpanKindClass::from_name`), `?since=<ns>`.
- Ordered `COALESCE(start_unix_ns, 0) DESC`.
- Limit clamped via `clamp(1, 1000)` from a default of `100`.

`get_span`: 404 if `(trace_id, span_id)` not found. Otherwise:
```
{"span": {... rich span row including "kind", "duration_ns", "status_message",
          "scope_name","scope_version","attributes" (parsed JSON), "resource" (parsed JSON or null), ...},
 "events": [{"event_pk": i64, "name": str, "time_unix_ns": i64, "attributes": Value}, ...],  // ASC by time
 "parent": { ... } | null,
 "children": [{"span_pk": i64, "trace_id": str, "span_id": str, "name": str, "kind_class": str}, ...],
 "projection": { ... }  // same shape as in span-tree node
}
```

`list_traces`: aggregate-per-trace.
```
{"traces": [
  {"trace_id": str, "first_seen_ns": i64, "last_seen_ns": i64,
   "span_count": i64, "placeholder_count": i64,
   "kind_counts": {"chat": i64, "execute_tool": i64, "external_tool": i64, "invoke_agent": i64, "other": i64},
   "root": {"span_pk": i64, "trace_id": str, "span_id": str, "parent_span_id": str?, "name": str, "kind_class": str, "ingestion_state": str} | null,
   "conversation_id": str?},
  ...
]}
```
- Filtered by `last_seen_ns >= since` and clamped limit (default 50, max 500).
- Sort: `(last_seen_ns = 0) DESC, last_seen_ns DESC` — placeholder-only traces (all spans null-timestamped) **float above** timestamped traces, and timestamped traces are newest-first.
- `conversation_id` is the first non-null `gen_ai.conversation.id` attribute across spans in the trace.

`get_trace`: 404 if no spans in trace. Otherwise `{"trace_id": str, "conversation_id": str?, "tree": [...]}` (tree built with same algorithm as `get_session_span_tree`).

`list_raw`:
```
{"raw": [
  {"id": i64, "received_at": str, "source": str, "record_type": str,
   "content_type": str?, "body": <parsed JSON OR raw string>}, ...
]}
```
- Filtered by `?type=<record_type>`.
- Limit clamped via `clamp(1, 500)` from a default of `100`. Ordered `id DESC`.
- `body` is `serde_json::from_str(&body)` if it parses, else the raw string.

## Limit-clamp matrix

| handler          | default | min | max  |
| ---------------- | ------: | --: | ---: |
| list_sessions    | 50      | 1   | 500  |
| list_traces      | 50      | 1   | 500  |
| list_raw         | 100     | 1   | 500  |
| list_spans       | 100     | 1   | 1000 |

`limit < 1` → 1; `limit > max` → max. Negative or zero values are clamped up.

## Suggested Test Strategy

- Mount the API router for end-to-end tests:
  ```rust
  use tower::ServiceExt;
  let app = ghcp_mon::server::api_router(state.clone());
  let resp = app.oneshot(Request::builder().uri("/api/healthz").body(Body::empty())?).await?;
  ```
  Or call individual handlers directly for unit-level coverage:
  ```rust
  let resp = ghcp_mon::api::list_spans(State(state), Query(ListQuery::default())).await?;
  ```
- **Pre-populate state**: open a real DB pool with `ghcp_mon::db::open(&tmp).await` so the schema is present, then `INSERT` rows directly via `sqlx::query` — this is the fastest way to set up table fixtures and avoids re-driving the whole normalize pipeline. Use the table specs in [[Normalize Pipeline Cheatsheet]] for column names. Foreign keys are enforced (per [[DB Module Cheatsheet]]); insert `raw_records` first, then `spans`, then projection tables.
- For **CORS** tests, see [[Server Router Cheatsheet]] (the layer is mounted on the router, not on individual handlers).
- For **local-workspace enrichment** tests (`list_sessions`, `get_session`):
  - Create an `AppState { session_state_dir_override: Arc::new(Some(<temp>)), .. }` pointing to a temp dir; place `<temp>/<cid>/workspace.yaml` with known fields. Assert `local_name`/`user_named`/`cwd`/`branch` round-trip.
  - With override `Arc::new(None)` and `$HOME`/`COPILOT_SESSION_STATE_DIR` env unset (or pointing to an empty temp), assert all four local fields are `null`.
- **Limit clamp**: hit each list handler with `?limit=0`, `?limit=999999`, `?limit=10`, no `?limit`, and confirm `LIMIT` resolves to clamp(default/0/999999/10). Fastest assertion: insert `>max` fake rows (e.g. 600) and confirm the response array length equals the upper clamp.
- **Kind filter**: insert spans with names spanning all five `SpanKindClass` buckets and confirm `?kind=chat` (etc.) returns only the matching bucket. The SQL CASE is a verbatim mirror of `SpanKindClass::from_name`.
- **Span-tree shape**: ingest two spans where the child arrives before the parent (use `[[Normalize Pipeline Cheatsheet]]` ingest pattern, **or** insert directly: a placeholder span then a real-state span). Hit `/api/sessions/:cid/span-tree` and confirm placeholder roots float to the top, children sort newest-first, and projection blocks appear when projection tables are populated.
- **Delete session**: pre-populate spans, projections, and a sessions row. Subscribe to `state.bus`. Call `delete_session`. Verify (a) all referenced rows are gone, (b) `trace_count` in the response equals the number of distinct trace ids, (c) one `EventMsg{kind:"derived", entity:"session", payload.action:"delete"}` was emitted.
- **list_raw body parsing**: insert one row with valid JSON body and one with a non-JSON body (e.g. `"hello"`); assert the first comes back as a JSON object/value, the second as a JSON string.
- **list_traces float-placeholder-only-traces-to-top**: insert two traces — A with all `start_unix_ns/end_unix_ns` set, B with all NULL. Confirm B appears first in the `traces` array.
- Use `axum::body::to_bytes(resp.into_body(), usize::MAX).await` then `serde_json::from_slice::<Value>` on responses.
- Don't mock `SqlitePool` or `Broadcaster` — both are cheap and using real ones reveals real wiring bugs.
