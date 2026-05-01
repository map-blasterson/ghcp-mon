# api — Memory Bank (backend)

## Purpose
The `src/api/` module is the backend's read/query plane: a JSON HTTP surface mounted under `/api` that lets the dashboard frontend (and external tooling) list and inspect normalized telemetry — sessions, traces, spans, projections, raw records, and per-conversation context snapshots. It is local-first and read-mostly: the only mutating endpoint is session deletion, which fans a tombstone event out over the WebSocket. All handlers translate database projections into stable JSON shapes and report failures through a single error envelope.

## Key behaviors

### Sessions
- `GET /api/sessions` returns up to `limit` rows from the `sessions` table whose `last_seen_ns >= since` (default `since=0`), ordered by `last_seen_ns DESC`; `limit` defaults to 50 and is clamped to `[1, 500]`. Each row carries `conversation_id`, `first_seen_ns`, `last_seen_ns`, `latest_model`, `chat_turn_count`, `tool_call_count`, and `agent_run_count`. `API list sessions ordered by recency`
- Each row in `GET /api/sessions` is enriched in-process with `local_name`, `user_named`, `cwd`, and `branch` read via `local_session::read_workspace_yaml(resolve_session_state_dir(state.session_state_dir_override), conversation_id)`; if the sidecar is missing or unreadable, all four fields are `null` and the row is still emitted. `API list sessions enriched with local workspace metadata`
- `GET /api/sessions/:cid` returns the `sessions` row plus a `span_count` equal to the number of spans whose `attributes_json` carries `gen_ai.conversation.id == :cid`; it returns 404 when no `sessions` row matches. `API session detail returns span count`
- The session detail body for `GET /api/sessions/:cid` includes `local_name`, `user_named`, `cwd`, `branch` from `workspace.yaml` (same `local_session` resolver as the list); when no sidecar is available, all four are `null`. `API session detail enriched with local workspace metadata`
- `GET /api/sessions/:cid/span-tree` builds a forest by seeding from every span carrying the conversation id (either via attribute or via an `agent_runs`/`chat_turns`/`tool_calls` row tagged with that conversation), unioning every span sharing a `trace_id` with any seed, and returning `{conversation_id, tree}` where each node has `span_pk`, `trace_id`, `span_id`, `parent_span_id`, `name`, `kind_class`, `ingestion_state`, `start_unix_ns`, `end_unix_ns`, a `projection` block, and recursively-nested `children`; siblings and roots are ordered placeholder/null-start first, then timestamped entries newest-first. `API session span tree trace scoped union`
- `GET /api/sessions/:cid/contexts` returns every `context_snapshots` row whose `conversation_id` matches, ordered by `captured_ns ASC`; each item carries `ctx_pk`, `span_pk`, `captured_ns`, `token_limit`, `current_tokens`, `messages_length`, the four token counters, and `source`. `API list session contexts ordered by capture`

### Traces & spans
- `GET /api/traces` returns one row per `trace_id` containing `first_seen_ns`, `last_seen_ns`, `span_count`, `placeholder_count`, a fixed-shape `kind_counts` map over the five kind classes, a `root` span object (NULL parent or parent not in the trace, earliest first), and the trace's `conversation_id` (any span carrying `gen_ai.conversation.id`). Filtering by `since` and `limit` clamping match the other list endpoints. `API list traces aggregates per trace`
- `GET /api/traces` orders results so traces with computed `last_seen_ns == 0` (placeholder-only traces with no timestamps yet) sort before traces with non-zero `last_seen_ns`; ties within each group break by `last_seen_ns DESC`. `API list traces floats placeholder only traces`
- `GET /api/traces/:trace_id` returns `{trace_id, conversation_id, tree}` using the same span-tree shape as the session span-tree, and returns 404 when no spans belong to the trace. `API get trace returns span tree`
- `GET /api/spans` returns up to `limit` spans (default 100, clamped to `[1, 1000]`) ordered by `start_unix_ns DESC`, optionally filtered by `since` (minimum `start_unix_ns`), `session` (matching either the span's own `gen_ai.conversation.id` attribute or any of its `agent_runs`/`chat_turns`/`tool_calls` projection rows), and `kind`. The `kind` filter is applied in SQL via a `CASE` over `name` mirroring `SpanKindClass::from_name` (`invoke_agent` for `name = 'invoke_agent'` or `'invoke_agent '`-prefixed; `chat` for `LIKE 'chat%'`; `execute_tool` for `LIKE 'execute_tool%'`; `external_tool` for `LIKE 'external_tool%'`; else `other`) and is applied before `LIMIT` so the page is never undercounted. `API list spans filterable by session and kind`
- `GET /api/spans/:trace_id/:span_id` returns 404 on miss; on hit it returns a `span` (attributes/resource parsed from JSON), an `events` array in `time_unix_ns ASC` order with parsed event attributes, the `parent` span when one exists, immediate `children` ordered by start time, and the span's `projection` block. `API get span returns events parent children projection`

### Raw records & contexts
- `GET /api/raw` returns up to `limit` rows from `raw_records` ordered by `id DESC` (default 100, clamped to `[1, 500]`), filtered by the `type` query parameter when present. Each item carries `id`, `received_at`, `source`, `record_type`, `content_type`, and `body`, where `body` is parsed JSON when valid JSON and otherwise the raw string. `API list raw filterable by record type`
- (Per-session context snapshots are listed under Sessions above: `API list session contexts ordered by capture`.)

### Lifecycle (delete + healthz)
- `DELETE /api/sessions/:cid` returns 404 when no `sessions` row matches; on hit it performs a single transactional purge: delete every `spans` row whose `trace_id` is reachable from the conversation (any span carrying `gen_ai.conversation.id == :cid`, or any span referenced by an `agent_runs`/`chat_turns`/`tool_calls` row tagged with that conversation), then delete remaining `context_snapshots`, `hook_invocations`, `skill_invocations`, `external_tool_calls`, `tool_calls`, `chat_turns`, `agent_runs`, and the `sessions` row tagged with `:cid`. After the transaction commits it publishes a `derived`/`session` event with `action: "delete"` and `conversation_id: :cid` via the WebSocket Broadcaster, and responds `{deleted: true, conversation_id, trace_count}`. `API delete session purges traces and projections`
- `GET /api/healthz` returns HTTP 200 with body `{"ok": true}`. `API healthz endpoint`

### Pagination & limits
- The shared `limit` helper used by every list endpoint clamps the requested limit to the inclusive range `[1, max]` and falls back to a per-endpoint default when the query parameter is absent. `API list query limit clamped`

## Public surface

HTTP routes hosted by `src/api/mod.rs` (mounted under `/api` by `src/server.rs`):

| Method | Path | Response shape |
| --- | --- | --- |
| GET | `/api/healthz` | `{"ok": true}` |
| GET | `/api/sessions` | `[ {conversation_id, first_seen_ns, last_seen_ns, latest_model, chat_turn_count, tool_call_count, agent_run_count, local_name, user_named, cwd, branch} ]` ordered `last_seen_ns DESC`; query: `since`, `limit` (default 50, clamp `[1,500]`) |
| GET | `/api/sessions/:cid` | session row + `span_count` + enrichment fields (`local_name`, `user_named`, `cwd`, `branch`); 404 on miss |
| GET | `/api/sessions/:cid/span-tree` | `{conversation_id, tree: [SpanNode]}` where each node = `{span_pk, trace_id, span_id, parent_span_id, name, kind_class, ingestion_state, start_unix_ns, end_unix_ns, projection, children}`; placeholder/null-start siblings first, then newest-first |
| GET | `/api/sessions/:cid/contexts` | `[ {ctx_pk, span_pk, captured_ns, token_limit, current_tokens, messages_length, <four token counters>, source} ]` ordered `captured_ns ASC` |
| DELETE | `/api/sessions/:cid` | `{deleted: true, conversation_id, trace_count}`; transactional purge + WS `derived/session action=delete`; 404 on miss |
| GET | `/api/traces` | `[ {trace_id, first_seen_ns, last_seen_ns, span_count, placeholder_count, kind_counts: {invoke_agent, chat, execute_tool, external_tool, other}, root, conversation_id} ]`; placeholder-only traces float to top; query: `since`, `limit` |
| GET | `/api/traces/:trace_id` | `{trace_id, conversation_id, tree}` (same span-tree shape as session span-tree); 404 on miss |
| GET | `/api/spans` | `[ Span ]` ordered `start_unix_ns DESC`; query: `since`, `session`, `kind` (filtered in SQL via `CASE` over `name`), `limit` (default 100, clamp `[1,1000]`) |
| GET | `/api/spans/:trace_id/:span_id` | `{span, events[], parent?, children[], projection}` with attributes/resource/event-attrs JSON-parsed; 404 on miss |
| GET | `/api/raw` | `[ {id, received_at, source, record_type, content_type, body} ]` ordered `id DESC`; `body` is parsed JSON when valid, else string; query: `type`, `limit` (default 100, clamp `[1,500]`) |

(`/api/replay`, `/api/sessions/:cid/registries`, and `/ws/events` are mounted by the same router but live in sibling modules per `API router exposes session and span endpoints`.)

## Invariants & constraints

- **Limit clamping policy.** Every list endpoint pipes its `limit` query parameter through a shared helper that clamps to `[1, max]` and applies a per-endpoint default; no endpoint serves an unclamped query (`API list query limit clamped`).
- **Default limit values.** Sessions: 50, max 500. Traces: same shape as other list endpoints. Spans: 100, max 1000. Raw records: 100, max 500.
- **404-on-miss.** `GET /api/sessions/:cid`, `DELETE /api/sessions/:cid`, `GET /api/traces/:trace_id`, and `GET /api/spans/:trace_id/:span_id` all return HTTP 404 when their primary key matches nothing; non-existence is not silently coerced to an empty body.
- **Transactional delete cascade.** Session purge happens inside a single SQL transaction covering `spans` (by reachable `trace_id`), then `context_snapshots`, `hook_invocations`, `skill_invocations`, `external_tool_calls`, `tool_calls`, `chat_turns`, `agent_runs`, and finally the `sessions` row. Only after commit does the handler publish the WS tombstone event.
- **Trace-scoped span-tree union.** Span-tree builders (both `/api/sessions/:cid/span-tree` and `/api/traces/:trace_id`) seed from every span/projection row tagged with the conversation id, then union every span sharing a `trace_id` with any seed. This is robust against orphan placeholders that lack timestamps and ensures a CLI session (= one trace) renders as one tree even when normalization is mid-flight.
- **Sibling/root ordering for trees.** Placeholder or null-start nodes always precede timestamped nodes; among timestamped nodes the order is newest-first.
- **Placeholder-only traces float.** `GET /api/traces` sorts traces with computed `last_seen_ns == 0` ahead of timestamped traces — fresh-but-still-placeholder traces are the newest data, not the oldest.
- **Kind classification in SQL.** The `kind` filter on `/api/spans` is implemented as a `CASE` expression over `name` that exactly mirrors `SpanKindClass::from_name`, applied in `WHERE` (before `LIMIT`) so a filtered page is never undercounted.
- **Enrichment fall-through.** Missing or unreadable `workspace.yaml` MUST NOT fail a request: `local_name`, `user_named`, `cwd`, `branch` are simply emitted as `null`. This applies to both the list and the detail handlers.
- **Authoritative span_count.** Session detail returns `span_count` computed from `attributes_json` (`gen_ai.conversation.id == :cid`) — not from the projection counters — because projection counters can lag normalization.
- **Raw body shape.** `body` in `/api/raw` items is parsed as JSON when the stored content is valid JSON and otherwise served as the raw string; the shape switch is per-row.
- **Error envelope.** Every handler returns errors via `AppError`, which renders as `{"error": "<message>"}` with status mapped by variant: `BadRequest → 400`, `NotFound → 404` (body `"not found"`), `NotImplemented → 501`, and `Sqlx`/`Migrate`/`Json`/`Io`/`Other → 500` (`AppError maps variants to status codes`, `AppError JSON body contains error message`).
- **Permissive CORS.** The router installs a CORS layer that allows any origin, any method, and any headers (`API allows any origin via CORS`). Note: the CORS layer is configured on the router built in `src/server.rs`, not inside `src/api/mod.rs` itself; the API module exports the route table that the server wraps.
- **Single router source of truth.** `src/api/mod.rs` is the canonical declaration of the dashboard URL surface (`API router exposes session and span endpoints`); changing the URL grammar requires updating this LLR alongside the code.

## Dependencies

- **Upstream consumers**
  - `web/src/api/client.ts` — the dashboard frontend wraps every route declared here.
  - External tooling and orchestrator liveness probes hit `/api/healthz`.
- **Downstream**
  - `db/` projection tables: `sessions`, `spans`, `agent_runs`, `chat_turns`, `tool_calls`, `external_tool_calls`, `hook_invocations`, `skill_invocations`, `context_snapshots`, `raw_records`.
  - `local_session::resolve_session_state_dir(state.session_state_dir_override)` + `local_session::read_workspace_yaml(...)` for session enrichment (best-effort, fall-through on error).
  - `ws/Broadcaster` — `DELETE /api/sessions/:cid` publishes a `derived`/`session` event with `action: "delete"` after the transactional purge commits.
  - `AppError` — uniform error envelope for every handler in this module.
- **Routing**
  - Mounted by `src/server.rs` under `/api`. The server is also responsible for the CORS layer and for composing the SPA fallback.

## Where to read for detail

### HLRs
- `backend/hlr/Dashboard REST API.md` — primary HLR; defines the API surface as a whole.
- `backend/hlr/Local Session Metadata.md` — co-parent for the two enrichment LLRs (best-effort `workspace.yaml` read).
- `backend/hlr/Telemetry Persistence.md` — co-parent for the delete-session purge (cascade across persisted projection tables).
- `backend/hlr/Live WebSocket Event Stream.md` — co-parent for the delete-session tombstone publish.
- `backend/hlr/Uniform Error Reporting.md` — co-parent for handlers that emit 404s and for the cross-cutting `AppError` envelope used by every handler in this module.

### LLRs (full enumeration, all under `backend/llr/`)
- `backend/llr/API list sessions ordered by recency.md`
- `backend/llr/API list sessions enriched with local workspace metadata.md`
- `backend/llr/API session detail returns span count.md`
- `backend/llr/API session detail enriched with local workspace metadata.md`
- `backend/llr/API session span tree trace scoped union.md`
- `backend/llr/API list session contexts ordered by capture.md`
- `backend/llr/API delete session purges traces and projections.md`
- `backend/llr/API list traces aggregates per trace.md`
- `backend/llr/API list traces floats placeholder only traces.md`
- `backend/llr/API get trace returns span tree.md`
- `backend/llr/API list spans filterable by session and kind.md`
- `backend/llr/API get span returns events parent children projection.md`
- `backend/llr/API list raw filterable by record type.md`
- `backend/llr/API list query limit clamped.md`
- `backend/llr/API healthz endpoint.md`

(Adjacent LLRs that bound this module from outside `src/api/mod.rs`: `backend/llr/API router exposes session and span endpoints.md` — the URL surface declaration mounted by `src/server.rs`; `backend/llr/API allows any origin via CORS.md` — CORS layer applied at the server level. Error envelope: `backend/llr/AppError maps variants to status codes.md`, `backend/llr/AppError JSON body contains error message.md`.)

### Source
- `src/api/mod.rs` — the implementation module; route declarations, handler bodies, span-tree builder, kind-classifier `CASE` expression, transactional delete, and `workspace.yaml` enrichment glue all live here.