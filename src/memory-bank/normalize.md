# normalize — Memory Bank (backend)

## Purpose
The `normalize` module is the single hop between accepted OTLP envelopes and the dashboard's queryable projections. Spans are treated as the canonical truth: every span is upserted idempotently, classified by name into a kind class, and projected into a kind-specific table (`agent_runs`, `chat_turns`, `tool_calls`, `external_tool_calls`) plus event-derived tables (`hook_invocations`, `skill_invocations`, `context_snapshots`). Conversation membership is inferred by walking the span ancestry, and a `sessions` row plus its denormalized counters are kept in sync per `conversation_id`. Out-of-order ingest is handled by a race-free placeholder pattern for unseen parents and by re-resolving descendant projection pointers whenever a parent finally arrives. Every persistent change emits an `EventMsg` to the WebSocket broadcaster so live UIs update without polling; logs are accepted but not normalized.

## Key behaviors

### Span upsert and placeholder state machine
- `normalize_span` upserts into `spans` keyed by `(trace_id, span_id)`: insert creates `ingestion_state='real'`; on conflict it overwrites mutable fields, forces `ingestion_state='real'`, and *coalesces* `resource_json`, `scope_name`, `scope_version` so partial re-deliveries never blank prior enrichment. `Span upsert by trace and span id`
- For any non-empty `parent_span_id`, the normalizer issues a single race-free `INSERT INTO spans … ON CONFLICT(trace_id, span_id) DO NOTHING RETURNING span_pk` for `(trace_id, parent_span_id)` with `name=''`, `attributes_json='{}'`, `ingestion_state='placeholder'`. Broadcast events fire **only when `RETURNING` produced a row**; the no-op conflict path is silent. `Placeholder span for unseen parent`
- When the real span for a key arrives after a placeholder, the upsert flips `ingestion_state` from `'placeholder'` to `'real'` and the broadcast carries `action="upgrade"` (versus `"insert"` when no prior row existed). The state machine is monotone — real never demotes to placeholder. `Placeholder upgrade preserved across reingest`
- On every span upsert (insert or conflict path), `DELETE FROM span_events WHERE span_pk = ?` is issued and the events on the current envelope are reinserted. Replays converge to the latest delivery rather than accumulating duplicates; deletion is scoped by `span_pk`, so other spans' events are untouched. `Span events idempotently replaced on span upsert`

### Span name kind-class classification
- `SpanKindClass::from_name(name)` (co-sourced in `model.rs`) maps span names to kind classes used to choose the projection table:
  - `InvokeAgent` if `name == "invoke_agent"` or starts with `"invoke_agent "`.
  - `Chat` if it starts with `"chat"`.
  - `ExecuteTool` if it starts with `"execute_tool"`.
  - `ExternalTool` if it starts with `"external_tool"`.
  - `Other` otherwise (no per-kind projection upserted).
  Span name is the **sole** input to projection routing; the rules are stable and explicit. `Span name classified into kind class`

### Per-kind projection upserts
- **InvokeAgent → `agent_runs`** (keyed by `span_pk`): populates `agent_name` from `gen_ai.agent.name` (falling back to the suffix after `invoke_agent ` in the span name), `agent_id` from `gen_ai.agent.id`, `agent_version` from `gen_ai.agent.version`, and `conversation_id` from `gen_ai.conversation.id`. On conflict pre-existing values are coalesced — late re-deliveries cannot erase prior enrichment. `Invoke agent span upserts agent run`
- **Chat → `chat_turns`** (keyed by `span_pk`): populates `conversation_id`, `interaction_id` (`github.copilot.interaction_id`), `turn_id` (`github.copilot.turn_id`), `model` (preferring `gen_ai.request.model` over `gen_ai.response.model`), and the four token-usage counters (`input_tokens`, `output_tokens`, `cache_read_tokens`, `reasoning_tokens`) from the matching `gen_ai.usage.*` attributes. `Chat span upserts chat turn`
- **ExecuteTool → `tool_calls`** (keyed by `span_pk`): populates `call_id` (`gen_ai.tool.call.id`), `tool_name` (`gen_ai.tool.name`), `tool_type` (`gen_ai.tool.type`), `conversation_id`, `start_unix_ns`/`end_unix_ns`/`duration_ns`, and `status_code`. `Execute tool span upserts tool call`
- **ExternalTool → `external_tool_calls`** (keyed by `span_pk`): `call_id` is taken from `github.copilot.external_tool.call_id` (falling back to `gen_ai.tool.call.id`); `tool_name` from `github.copilot.external_tool.name` (falling back to `gen_ai.tool.name`). External-tool spans use a copilot-specific attribute namespace that aliases the generic `gen_ai` keys. `External tool span upserts external tool call`
- **Pairing internal ↔ external** by `call_id`: when a `tool_calls` row is upserted with non-null `call_id`, every `external_tool_calls` row with the same `call_id` whose `paired_tool_call_pk` is null is updated to point at the new `tool_call_pk`. Symmetrically, an `external_tool_calls` upsert looks up the matching `tool_calls.tool_call_pk` by `call_id` and stores it as `paired_tool_call_pk` if available. The two sides arrive in either order; pairing lets the UI render a single tool call. `External tool paired to internal tool call by call id`

### Conversation id inheritance and sessions
- `effective_conversation_id(span_pk)` returns `gen_ai.conversation.id` from the closest ancestor (including the span itself, depth 0) whose `attributes_json` carries that attribute, walking up to **64 levels**. Returns `None` if no ancestor in the chain carries it. Tool/chat spans usually carry the id directly, while nested `invoke_agent` children must inherit it from their parent. `Effective conversation id inherited from ancestors`
- When a span resolves to a non-null effective conversation id, a `sessions` row keyed by `conversation_id` is upserted with `first_seen_ns = MIN(existing, new)`, `last_seen_ns = MAX(existing, new)`, and `latest_model` coalescing to the just-observed model when present. `Session upserted per conversation id`
- After the `sessions` upsert, `chat_turn_count`, `tool_call_count`, and `agent_run_count` are recomputed for that `conversation_id` from the corresponding projection tables and stored on the row, so the dashboard's session list reads counters directly. `Session counters refreshed on session upsert`

### Pointer reconciliation (ancestor walk + forward resolve)
- After upserting a span and its projection row, the normalizer walks the span's ancestors (joining `spans` on `trace_id` + `parent_span_id`) **up to depth 64** and back-fills flat parent pointers using `COALESCE` (so existing values are never overwritten):
  - `agent_runs.parent_agent_run_pk`, `agent_runs.parent_span_pk`.
  - `agent_run_pk`, `chat_turn_pk`, `conversation_id` on the row's `chat_turns`, `tool_calls`, `external_tool_calls`, `hook_invocations`, `skill_invocations` projections.
  Projections store flat parent pointers for cheap querying, derived from the recursive parent chain. `Projection pointers resolved via ancestor walk`
- After resolving its own pointers, the normalizer recursively re-resolves projection pointers for **every descendant** of the just-ingested span, **up to depth 128**. This is the out-of-order recovery path: children that were ingested before their parent get their pointers populated when the parent finally arrives. `Forward resolve descendants on parent arrival`

### Event lineage (hooks, skills, usage, token counts)
- `github.copilot.hook.start` events upsert a `hook_invocations` row keyed by `invocation_id` (from `github.copilot.hook.invocation_id`), recording `hook_type`, `span_pk`, `conversation_id`, and `start_unix_ns` from the event's time. `conversation_id` is coalesced on conflict. `Hook start event derives hook invocation`
- `github.copilot.hook.end` events upsert the matching `hook_invocations` row by `invocation_id`, setting `end_unix_ns` and computing `duration_ns = end_unix_ns - start_unix_ns` **only when `start_unix_ns` is already set** (duration is meaningful only after both ends are observed). `Hook end event completes hook invocation`
- `github.copilot.skill.invoked` events insert a `skill_invocations` row carrying `span_pk`, `skill_name`, `skill_path`, `invoked_unix_ns`, and `conversation_id`. The unique key `(span_pk, invoked_unix_ns, skill_name)` does nothing on conflict — replaying the same span never duplicates rows. `Skill invoked event records skill invocation`
- `github.copilot.session.usage_info` events upsert a `context_snapshots` row with `source='usage_info_event'` keyed by `(span_pk, source, captured_ns)`, capturing `token_limit`, `current_tokens`, `messages_length` from event attributes; each field coalesces on conflict. `Usage info event creates context snapshot`
- A `Chat` span carrying any of `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.usage.cache_read.input_tokens`, or `gen_ai.usage.reasoning.output_tokens` upserts a `context_snapshots` row with `source='chat_span'`, `captured_ns = end_unix_ns ?? start_unix_ns`, and the four token counters; each coalesces on conflict against `(span_pk, source, captured_ns)`. This is a derived per-turn snapshot supplementing the raw `chat_turns` row for time-series displays. `Chat token usage attributes create context snapshot`
- After projection updates, `tool_call_count` is refreshed on every `chat_turns.turn_pk` reachable from the just-ingested span — directly via `chat_turns.span_pk = span_pk`, or indirectly via `tool_calls.span_pk = span_pk` / `external_tool_calls.span_pk = span_pk` — by counting `tool_calls` rows whose `chat_turn_pk` matches. A new tool span anywhere under a chat turn updates that turn's denormalized counter. `Chat turn tool count refreshed`

### Metrics and logs
- For each data point of a metric envelope, one row is inserted into `metric_points` carrying `raw_record_id`, `metric_name`, `description`, `unit`, optional `start_unix_ns`/`end_unix_ns`, JSON-encoded `attributes_json`/`value_json`, and the envelope's `resource_json`/`scope_name`/`scope_version`. Metric data is stored point-by-point so each is independently queryable. `Metric data points persisted to metric_points`
- `handle_envelope` does **not** produce any normalized rows for `Envelope::Log`; it returns `Ok(())` after no-op handling, leaving log persistence to the raw-record archive. Normalization for logs is intentionally deferred. `Logs not normalized currently`

### Broadcast emission
- After upserting a (real) span, two `EventMsg` records are broadcast: a `kind="span"`, `entity="span"` event carrying `action`, `trace_id`, `span_id`, `parent_span_id`, `name`, `kind_class`, `ingestion_state="real"`, `span_pk`; and a `kind="trace"`, `entity="trace"` event carrying `action`, `trace_id`, `span_id`, `ingestion_state="real"`, and `upgraded` (true iff a placeholder was upgraded). `Span normalize emits span and trace events`
- On *creating* a placeholder row (i.e., the idempotent insert actually inserted), the normalizer emits a `kind="span"`, `entity="placeholder"` event with `action="insert"`, `trace_id`, `span_id`, `span_pk`, plus a `kind="trace"`, `entity="trace"` event with `action="placeholder"` and `ingestion_state="placeholder"`. When the placeholder insert is a no-op (row already exists), **no events fire**. `Placeholder creation emits placeholder events`
- After upserting an `agent_runs`, `chat_turns`, or `tool_calls` row, a `kind="derived"` event is emitted with `entity` = `"agent_run"` / `"chat_turn"` / `"tool_call"`, carrying the projection ids (`agent_run_pk` / `turn_pk` / `tool_call_pk`), `span_pk`, `conversation_id`, and projection-specific fields (`agent_name`; `interaction_id`/`turn_id`; `tool_name`/`call_id`/`status_code`). Per-projection events let the UI update specific tables without re-querying. `Projection upserts emit derived events`
- On every `sessions` upsert a `kind="derived"`, `entity="session"` event is broadcast with `action="update"`, `conversation_id`, and `latest_model`. The legal `action` values for `derived`/`session` are `update` (here) and `delete` (from API session deletion); producers must not emit other values. `Session upsert emits derived session event`
- After persisting a metric envelope, a `kind="metric"`, `entity="metric"` event is broadcast whose payload contains `name` and `points` (the data-point count). The event is summary-only; full metric data is fetched on demand. `Metric ingest emits raw metric event`

## Public surface
- `handle_envelope(envelope, …)` — single dispatcher invoked by ingest; matches on `Envelope::Span` / `Envelope::Metric` / `Envelope::Log` (the last is a no-op). `Logs not normalized currently`
- `normalize_span(…)` — span entry that performs the upsert + projection + reconciliation pipeline. `Span upsert by trace and span id`
- `effective_conversation_id(span_pk)` — ancestor-walk helper, depth 64. `Effective conversation id inherited from ancestors`
- Ancestor-walk pointer reconcile (depth 64) and forward-resolve descendant reconcile (depth 128) — internal helpers; read source for exact names. `Projection pointers resolved via ancestor walk`, `Forward resolve descendants on parent arrival`
- `SpanKindClass::from_name(name)` — co-sourced from `src/model.rs`; classifies span name into `InvokeAgent` / `Chat` / `ExecuteTool` / `ExternalTool` / `Other`. `Span name classified into kind class`
- `EventMsg` payload shapes per emission LLR:
  - `kind="span"`, `entity="span"` — `action`, `trace_id`, `span_id`, `parent_span_id`, `name`, `kind_class`, `ingestion_state="real"`, `span_pk`. `Span normalize emits span and trace events`
  - `kind="trace"`, `entity="trace"` — `action`, `trace_id`, `span_id`, `ingestion_state` (`"real"` or `"placeholder"`), `upgraded` on real path. `Span normalize emits span and trace events`, `Placeholder creation emits placeholder events`
  - `kind="span"`, `entity="placeholder"` — `action="insert"`, `trace_id`, `span_id`, `span_pk`. `Placeholder creation emits placeholder events`
  - `kind="derived"`, `entity` ∈ `{"agent_run","chat_turn","tool_call"}` — projection ids, `span_pk`, `conversation_id`, projection-specific fields. `Projection upserts emit derived events`
  - `kind="derived"`, `entity="session"` — `action="update"` (or `"delete"` from API), `conversation_id`, `latest_model`. `Session upsert emits derived session event`
  - `kind="metric"`, `entity="metric"` — `name`, `points`. `Metric ingest emits raw metric event`

## Invariants & constraints
- `(trace_id, span_id)` is the natural key for `spans`; `span_pk` is the surrogate used by every projection's foreign key. `Span upsert by trace and span id`
- `ingestion_state` is monotone: `placeholder → real` on upgrade; `real` is never demoted to `placeholder`. `Placeholder upgrade preserved across reingest`
- Placeholder insert is race-free via `INSERT … ON CONFLICT(trace_id, span_id) DO NOTHING RETURNING span_pk`; broadcast emission is gated on `RETURNING` producing a row, so no-op conflict paths emit nothing. `Placeholder span for unseen parent`, `Placeholder creation emits placeholder events`
- Span event sub-rows are wholesale replaced on every span upsert (`DELETE … WHERE span_pk = ?` then re-insert) — replays do not accumulate duplicates and deletion is scoped to the span. `Span events idempotently replaced on span upsert`
- Optional fields (`resource_json`, `scope_name`, `scope_version`, plus all projection enrichment fields) coalesce on conflict so partial re-deliveries cannot blank existing data. `Span upsert by trace and span id`, `Invoke agent span upserts agent run`
- Effective conversation id is inherited via ancestor walk; max depth **64**. `Effective conversation id inherited from ancestors`
- Pointer reconcile depths: ancestor walk **64** up; descendant forward-resolve **128** down. `Projection pointers resolved via ancestor walk`, `Forward resolve descendants on parent arrival`
- Pointer back-fills always use `COALESCE`: a later span never overwrites an existing parent pointer. `Projection pointers resolved via ancestor walk`
- Internal/external tool pairing by `call_id` is bidirectional and order-independent — pair from either side as soon as the partner exists. `External tool paired to internal tool call by call id`
- `hook_invocations.duration_ns` is computed only when `start_unix_ns` is already set; end-only deliveries leave duration null. `Hook end event completes hook invocation`
- `skill_invocations` uses `ON CONFLICT (span_pk, invoked_unix_ns, skill_name) DO NOTHING` for idempotent replay. `Skill invoked event records skill invocation`
- `context_snapshots` is keyed by `(span_pk, source, captured_ns)` with `source ∈ {usage_info_event, chat_span}`; counters coalesce on conflict. `Usage info event creates context snapshot`, `Chat token usage attributes create context snapshot`
- `chat_turns.tool_call_count` is denormalized and refreshed for every reachable `turn_pk` after each span upsert (direct chat span, or via tool / external-tool span). `Chat turn tool count refreshed`
- Sessions: `first_seen_ns = MIN(existing, new)`, `last_seen_ns = MAX(existing, new)`, `latest_model` coalesces; the three counters (`chat_turn_count`, `tool_call_count`, `agent_run_count`) are recomputed from the projection tables after each upsert. `Session upserted per conversation id`, `Session counters refreshed on session upsert`
- Logs are intentionally NOT normalized — `Envelope::Log` is a no-op `Ok(())`. `Logs not normalized currently`
- Metrics are stored point-by-point; metric broadcast carries only `name` and `points` count, never the data itself. `Metric data points persisted to metric_points`, `Metric ingest emits raw metric event`
- Legal `action` values for `derived`/`session`: only `update` (this module) and `delete` (API delete path). Producers must not emit others. `Session upsert emits derived session event`
- Broadcast send errors are ignored — the no-subscriber case is tolerated (see `ws.md`).

## Dependencies
- **Reads:** internal envelope shapes from `src/model.rs` — `SpanEnvelope`, `MetricEnvelope`, `LogEnvelope`; also consumes `SpanKindClass::from_name` co-sourced there. `Span name classified into kind class`
- **Writes:** every projection table via `db/` DAO calls / sqlx —
  - `spans`, `span_events` (canonical truth + per-span event sub-rows). `Span upsert by trace and span id`, `Span events idempotently replaced on span upsert`
  - `agent_runs`, `chat_turns`, `tool_calls`, `external_tool_calls` (per-kind projections). `Invoke agent span upserts agent run`, `Chat span upserts chat turn`, `Execute tool span upserts tool call`, `External tool span upserts external tool call`
  - `hook_invocations`, `skill_invocations`, `context_snapshots` (event-derived). `Hook start event derives hook invocation`, `Skill invoked event records skill invocation`, `Usage info event creates context snapshot`
  - `sessions` (per-conversation aggregation, with refreshed counters). `Session upserted per conversation id`, `Session counters refreshed on session upsert`
  - `metric_points` (one row per metric data point). `Metric data points persisted to metric_points`
- **Sends:** `EventMsg` records to `ws::Broadcaster` (span / trace / placeholder / derived-{agent_run,chat_turn,tool_call,session} / metric); send errors are ignored.
- **Returns:** `Result<_, AppError>` — failures from any DB step or attribute extraction surface to the ingest caller as `AppError`.

## Where to read for detail
- HLRs:
  - `backend/hlr/Span Normalization.md`
  - `backend/hlr/Live WebSocket Event Stream.md` (broadcast emission cross-cuts)
- LLRs (all 27):
  - Span upsert / placeholder / state machine:
    - `backend/llr/Span upsert by trace and span id.md`
    - `backend/llr/Placeholder span for unseen parent.md`
    - `backend/llr/Placeholder upgrade preserved across reingest.md`
    - `backend/llr/Span events idempotently replaced on span upsert.md`
    - `backend/llr/Span name classified into kind class.md`
  - Per-kind projection upserts:
    - `backend/llr/Invoke agent span upserts agent run.md`
    - `backend/llr/Chat span upserts chat turn.md`
    - `backend/llr/Execute tool span upserts tool call.md`
    - `backend/llr/External tool span upserts external tool call.md`
    - `backend/llr/External tool paired to internal tool call by call id.md`
  - Sessions / conversation id:
    - `backend/llr/Effective conversation id inherited from ancestors.md`
    - `backend/llr/Session upserted per conversation id.md`
    - `backend/llr/Session counters refreshed on session upsert.md`
  - Pointer reconciliation:
    - `backend/llr/Projection pointers resolved via ancestor walk.md`
    - `backend/llr/Forward resolve descendants on parent arrival.md`
  - Event lineage:
    - `backend/llr/Hook start event derives hook invocation.md`
    - `backend/llr/Hook end event completes hook invocation.md`
    - `backend/llr/Skill invoked event records skill invocation.md`
    - `backend/llr/Usage info event creates context snapshot.md`
    - `backend/llr/Chat token usage attributes create context snapshot.md`
    - `backend/llr/Chat turn tool count refreshed.md`
  - Metrics & logs:
    - `backend/llr/Metric data points persisted to metric_points.md`
    - `backend/llr/Logs not normalized currently.md`
  - Broadcast emission:
    - `backend/llr/Span normalize emits span and trace events.md`
    - `backend/llr/Placeholder creation emits placeholder events.md`
    - `backend/llr/Projection upserts emit derived events.md`
    - `backend/llr/Session upsert emits derived session event.md`
    - `backend/llr/Metric ingest emits raw metric event.md`
- Source: `src/normalize/mod.rs` (27 of 27).
- Co-sourced: `src/model.rs` (`SpanKindClass::from_name`).