# Backend Database Schema (from vault)

## Overview

- **Engine:** SQLite, accessed via `sqlx::SqlitePool` over a `sqlite://<path>?mode=rwc` URL with `create_if_missing=true`, `journal_mode=WAL`, `synchronous=Normal`, `foreign_keys=true`, and a max pool of 8 connections (`backend/__cheatsheets/DB Module Cheatsheet.md`; `backend/llr/DB open enables WAL and foreign keys.md`).
- **File location:** default `./data/ghcp-mon.db`, overridable via the global `--db <path>` CLI flag; `db::open` creates the parent directory recursively if missing (`backend/llr/CLI db option default path.md`; `backend/llr/DB open creates parent directory.md`).
- **Connection model:** single async pool shared across the OTLP receiver, normalizer, REST API, and replay paths. Foreign keys are enforced at the connection level, so all inserts that reference `raw_records` or `spans` require the parent row to exist (`backend/__cheatsheets/Normalize Pipeline Cheatsheet.md` setup notes; `backend/__cheatsheets/REST API Cheatsheet.md`).
- **Schema source:** the schema is owned entirely by migrations in `./migrations/` (relative to crate root), embedded at compile time via `sqlx::migrate!("./migrations")` and re-applied on every `db::open` (`backend/__cheatsheets/DB Module Cheatsheet.md`; `backend/llr/DB open runs migrations.md`).

The vault never quotes the migration SQL itself — table descriptions below are reverse-engineered from cheatsheets and LLRs that name the columns these tables expose.

## Tables

The fixed reference for column names is `backend/__cheatsheets/Normalize Pipeline Cheatsheet.md` ("Extract — observable database surface"), confirmed against `backend/__cheatsheets/DAO Cheatsheet.md` and the per-projection LLRs. Types are inferred from behaviour (e.g. `_ns`/`_pk`/`_count` → INTEGER, `*_json` → TEXT JSON, attribute-bearing strings → TEXT).

### `raw_records`

- **Purpose:** Verbatim archive of every payload accepted by the system — one row per OTLP/HTTP request body and one row per replayed envelope. All normalized rows reference it via `raw_record_id` so projections can be traced back to the bytes that produced them (`backend/llr/Raw request body persisted verbatim per OTLP request.md`; `backend/llr/Each envelope persisted as own raw record.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `id` | INTEGER | no | AUTOINCREMENT | PK; returned by `dao::insert_raw` via `RETURNING id` |
  | `received_at` | (timestamp) | no | server-side default | listed by `list_raw` as a string field; default not pinned in the vault |
  | `source` | TEXT | no | — | e.g. `'otlp-http-json'` for OTLP, replay sets its own source string |
  | `record_type` | TEXT | no | — | envelope type tag: `'span'` / `'metric'` / `'log'`, or per-OTLP `record_type` |
  | `content_type` | TEXT | yes | NULL | e.g. `'application/json'`; OTLP sets it explicitly |
  | `body` | TEXT | no | — | raw request body, stored verbatim |

- **Indexes:** none documented in the vault.
- **Constraints:** none beyond the PK; referenced by FK from `span_events.raw_record_id`, `spans.first_seen_raw_id` / `last_seen_raw_id`, and `metric_points.raw_record_id`.
- **Lifecycle:** Inserted by `dao::insert_raw` / `ingest::persist_raw_request` once per OTLP request and once per replayed envelope. Never mutated. The vault does not specify a purge/retention policy (`backend/__cheatsheets/DAO Cheatsheet.md`; `backend/__cheatsheets/Ingest Pipeline Cheatsheet.md`).
- **Vault refs:** `[[Telemetry Persistence]]`, `[[DAO insert_raw returns row id]]`, `[[Raw request body persisted verbatim per OTLP request]]`, `[[Each envelope persisted as own raw record]]`, `[[DAO Cheatsheet]]`.

### `spans`

- **Purpose:** Canonical normalized span store — one row per `(trace_id, span_id)`. Both real spans and "placeholder" rows for unseen parents live here (`backend/llr/Span upsert by trace and span id.md`; `backend/llr/Placeholder span for unseen parent.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `span_pk` | INTEGER | no | AUTOINCREMENT | PK |
  | `trace_id` | TEXT | no | — | part of UNIQUE `(trace_id, span_id)` |
  | `span_id` | TEXT | no | — | part of UNIQUE `(trace_id, span_id)` |
  | `parent_span_id` | TEXT | yes | NULL | non-empty parent only; empty strings dropped to NULL by ingest |
  | `name` | TEXT | no | `''` for placeholders | placeholder rows insert `name=''` |
  | `kind` | INTEGER | yes | NULL | OTLP span kind enum |
  | `start_unix_ns` | INTEGER | yes | NULL | NULL on placeholder rows |
  | `end_unix_ns` | INTEGER | yes | NULL | |
  | `duration_ns` | INTEGER | yes | NULL | derived |
  | `status_code` | INTEGER | yes | NULL | from `SpanStatus.code` |
  | `status_message` | TEXT | yes | NULL | from `SpanStatus.message` |
  | `attributes_json` | TEXT (JSON) | no | `'{}'` for placeholders | flattened OTLP attrs serialized |
  | `resource_json` | TEXT (JSON) | yes | NULL | coalesced on conflict |
  | `scope_name` | TEXT | yes | NULL | coalesced on conflict |
  | `scope_version` | TEXT | yes | NULL | coalesced on conflict |
  | `ingestion_state` | TEXT | no | — | enum `'real' \| 'placeholder'`; forced to `'real'` on real-span upsert |
  | `first_seen_raw_id` | INTEGER | yes | NULL | FK → `raw_records.id` |
  | `last_seen_raw_id` | INTEGER | yes | NULL | FK → `raw_records.id` |

- **Indexes:** `UNIQUE (trace_id, span_id)` (used as the `ON CONFLICT` target).
- **Constraints:** Implicit FK from `span_events.span_pk`, `agent_runs.span_pk`, `chat_turns.span_pk`, `tool_calls.span_pk`, `external_tool_calls.span_pk`, `hook_invocations.span_pk`, `skill_invocations.span_pk`, `context_snapshots.span_pk`. `delete_session` deletes `spans` rows by `trace_id IN (...)` "and cascades to `span_events` and projection rows tagged by FK", which implies `ON DELETE CASCADE` on at least `span_events` (and likely the projections), per `backend/__cheatsheets/REST API Cheatsheet.md`.
- **Lifecycle:** Inserted/upserted by `normalize_span` keyed by `(trace_id, span_id)`; placeholders inserted by parent-resolution path with `ingestion_state='placeholder'`, then upgraded in place to `'real'` on later real arrival. Optional fields are coalesced (never blanked) on conflict. Deleted en masse by `DELETE /api/sessions/:cid` for every trace reachable from the conversation (`backend/llr/API delete session purges traces and projections.md`).
- **Vault refs:** `[[Span upsert by trace and span id]]`, `[[Placeholder span for unseen parent]]`, `[[Placeholder upgrade preserved across reingest]]`, `[[Span normalize emits span and trace events]]`, `[[Normalize Pipeline Cheatsheet]]`.

### `span_events`

- **Purpose:** OTLP span events for each span, used for hook/skill/usage-info derivations and shown in `GET /api/spans/:trace/:span` (`backend/__cheatsheets/REST API Cheatsheet.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `event_pk` | INTEGER | no | AUTOINCREMENT | PK |
  | `span_pk` | INTEGER | no | — | FK → `spans.span_pk` |
  | `raw_record_id` | INTEGER | no | — | FK → `raw_records.id` |
  | `name` | TEXT | no | — | event name |
  | `time_unix_ns` | INTEGER | no | — | from `EventEnvelope.time` |
  | `attributes_json` | TEXT (JSON) | no | `'{}'` | flattened |

- **Indexes:** none documented; lookup by `span_pk` and ordered by `time_unix_ns ASC` for `get_span` (`backend/__cheatsheets/REST API Cheatsheet.md`).
- **Constraints:** likely `ON DELETE CASCADE` from `spans` (see `delete_session` cascade note).
- **Lifecycle:** Idempotently replaced on every span upsert: `DELETE FROM span_events WHERE span_pk = ?` followed by re-insertion of the current envelope's events, scoped to the upserted `span_pk` (`backend/llr/Span events idempotently replaced on span upsert.md`).
- **Vault refs:** `[[Span events idempotently replaced on span upsert]]`, `[[Hook start event derives hook invocation]]`, `[[Skill invoked event records skill invocation]]`.

### `agent_runs`

- **Purpose:** Projection row for `invoke_agent`-class spans — one per agent invocation, plus parent pointers (`backend/llr/Invoke agent span upserts agent run.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `agent_run_pk` | INTEGER | no | AUTOINCREMENT | PK |
  | `span_pk` | INTEGER | no | — | UNIQUE; FK → `spans.span_pk` |
  | `conversation_id` | TEXT | yes | NULL | from `gen_ai.conversation.id` (or inherited) |
  | `agent_id` | TEXT | yes | NULL | `gen_ai.agent.id` |
  | `agent_name` | TEXT | yes | NULL | `gen_ai.agent.name`, fallback to span-name suffix after `invoke_agent ` |
  | `agent_version` | TEXT | yes | NULL | `gen_ai.agent.version` |
  | `start_unix_ns` | INTEGER | yes | NULL | mirrored from span |
  | `end_unix_ns` | INTEGER | yes | NULL | mirrored from span |
  | `duration_ns` | INTEGER | yes | NULL | mirrored from span |
  | `parent_agent_run_pk` | INTEGER | yes | NULL | resolved by ancestor walk |
  | `parent_span_pk` | INTEGER | yes | NULL | resolved by ancestor walk |

- **Indexes / constraints:** UNIQUE on `span_pk`; values coalesced (`COALESCE(existing, new)`) on conflict so re-deliveries never erase enrichment.
- **Lifecycle:** Upserted by `upsert_agent_run`. `parent_*_pk` filled by `walk_ancestors` (depth ≤ 64) initially and refilled when descendants are reconciled after a late parent arrival (`backend/llr/Projection pointers resolved via ancestor walk.md`; `backend/llr/Forward resolve descendants on parent arrival.md`). Counted by `sessions.agent_run_count` refresh. Deleted by `delete_session` for the matching `conversation_id`.
- **Vault refs:** `[[Invoke agent span upserts agent run]]`, `[[Projection pointers resolved via ancestor walk]]`, `[[Forward resolve descendants on parent arrival]]`.

### `chat_turns`

- **Purpose:** One row per `chat`-class span, holding per-turn token accounting, model, and parent pointers (`backend/llr/Chat span upserts chat turn.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `turn_pk` | INTEGER | no | AUTOINCREMENT | PK |
  | `span_pk` | INTEGER | no | — | UNIQUE; FK → `spans.span_pk` |
  | `conversation_id` | TEXT | yes | NULL | `gen_ai.conversation.id` |
  | `interaction_id` | TEXT | yes | NULL | `github.copilot.interaction_id` |
  | `turn_id` | TEXT | yes | NULL | `github.copilot.turn_id` |
  | `model` | TEXT | yes | NULL | prefers `gen_ai.request.model` over `gen_ai.response.model` |
  | `input_tokens` | INTEGER | yes | NULL | `gen_ai.usage.input_tokens` |
  | `output_tokens` | INTEGER | yes | NULL | `gen_ai.usage.output_tokens` |
  | `cache_read_tokens` | INTEGER | yes | NULL | `gen_ai.usage.cache_read.input_tokens` |
  | `reasoning_tokens` | INTEGER | yes | NULL | `gen_ai.usage.reasoning.output_tokens` |
  | `tool_call_count` | INTEGER | no | 0 | refreshed from `tool_calls` only (not external_tool_calls) |
  | `agent_run_pk` | INTEGER | yes | NULL | resolved via ancestor walk |
  | `start_unix_ns` | INTEGER | yes | NULL | |
  | `end_unix_ns` | INTEGER | yes | NULL | |

- **Indexes / constraints:** UNIQUE on `span_pk`.
- **Lifecycle:** Upserted by `upsert_chat_turn`. `tool_call_count` recomputed by `refresh_chat_turn_tool_count_for_span` after each span normalization, counting only `tool_calls` rows (not `external_tool_calls`) with matching `chat_turn_pk` (`backend/llr/Chat turn tool count refreshed.md`; `backend/__cheatsheets/Normalize Pipeline Cheatsheet.md`).
- **Vault refs:** `[[Chat span upserts chat turn]]`, `[[Chat turn tool count refreshed]]`, `[[Chat token usage attributes create context snapshot]]`.

### `tool_calls`

- **Purpose:** Projection row for `execute_tool`-class spans (internal/local tool invocations) (`backend/llr/Execute tool span upserts tool call.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `tool_call_pk` | INTEGER | no | AUTOINCREMENT | PK |
  | `span_pk` | INTEGER | no | — | UNIQUE; FK → `spans.span_pk` |
  | `call_id` | TEXT | yes | NULL | `gen_ai.tool.call.id` |
  | `tool_name` | TEXT | yes | NULL | `gen_ai.tool.name` |
  | `tool_type` | TEXT | yes | NULL | `gen_ai.tool.type` |
  | `conversation_id` | TEXT | yes | NULL | inherited via ancestor walk |
  | `start_unix_ns` | INTEGER | yes | NULL | |
  | `end_unix_ns` | INTEGER | yes | NULL | |
  | `duration_ns` | INTEGER | yes | NULL | |
  | `status_code` | INTEGER | yes | NULL | from span |
  | `agent_run_pk` | INTEGER | yes | NULL | resolved via ancestor walk |
  | `chat_turn_pk` | INTEGER | yes | NULL | resolved via ancestor walk |

- **Indexes / constraints:** UNIQUE on `span_pk`. After upsert, the normalizer also updates pre-existing `external_tool_calls` rows with matching `call_id` (and NULL `paired_tool_call_pk`), setting their `paired_tool_call_pk = this.tool_call_pk` (`backend/llr/External tool paired to internal tool call by call id.md`).
- **Lifecycle:** Upserted by `upsert_tool_call`. Counted by `chat_turns.tool_call_count` refresh and `sessions.tool_call_count` refresh. Purged by `delete_session`.
- **Vault refs:** `[[Execute tool span upserts tool call]]`, `[[External tool paired to internal tool call by call id]]`.

### `external_tool_calls`

- **Purpose:** Projection row for `external_tool`-class spans, paired (when possible) to a sibling internal `tool_calls` row by `call_id` (`backend/llr/External tool span upserts external tool call.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `ext_pk` | INTEGER | no | AUTOINCREMENT | PK |
  | `span_pk` | INTEGER | no | — | UNIQUE; FK → `spans.span_pk` |
  | `call_id` | TEXT | yes | NULL | `github.copilot.external_tool.call_id` (fallback `gen_ai.tool.call.id`) |
  | `tool_name` | TEXT | yes | NULL | `github.copilot.external_tool.name` (fallback `gen_ai.tool.name`) |
  | `paired_tool_call_pk` | INTEGER | yes | NULL | FK → `tool_calls.tool_call_pk`; set lazily |
  | `conversation_id` | TEXT | yes | NULL | resolved via ancestor walk |
  | `start_unix_ns` | INTEGER | yes | NULL | |
  | `end_unix_ns` | INTEGER | yes | NULL | |
  | `duration_ns` | INTEGER | yes | NULL | |
  | `agent_run_pk` | INTEGER | yes | NULL | resolved via ancestor walk |
  | `chat_turn_pk` | INTEGER | yes | NULL | resolved via ancestor walk |

- **Indexes / constraints:** UNIQUE on `span_pk`. Pairing is bidirectional (set on this row when an internal `tool_calls` row exists; set on existing rows here when an internal `tool_calls` row arrives later).
- **Lifecycle:** Upserted by `upsert_external_tool_call`; pairing updated on either order of arrival. Not counted in `chat_turns.tool_call_count`. Purged by `delete_session`.
- **Vault refs:** `[[External tool span upserts external tool call]]`, `[[External tool paired to internal tool call by call id]]`.

### `sessions`

- **Purpose:** User-visible aggregation per `conversation_id` — one row per logical chat session, with denormalized counters and time bounds (`backend/llr/Session upserted per conversation id.md`; `backend/llr/Session counters refreshed on session upsert.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `conversation_id` | TEXT | no | — | PK |
  | `first_seen_ns` | INTEGER | yes | NULL | `MIN(existing, new)` on conflict |
  | `last_seen_ns` | INTEGER | yes | NULL | `MAX(existing, new)` on conflict |
  | `latest_model` | TEXT | yes | NULL | `COALESCE(new, existing)` |
  | `chat_turn_count` | INTEGER | no | 0 | recomputed from `chat_turns` |
  | `tool_call_count` | INTEGER | no | 0 | recomputed from `tool_calls` (not external) |
  | `agent_run_count` | INTEGER | no | 0 | recomputed from `agent_runs` |

- **Indexes / constraints:** PK on `conversation_id`. (No FK from projections — projection rows hold `conversation_id` as a free string.)
- **Lifecycle:** Upserted by `upsert_session_for_span` whenever a real span resolves to a non-null effective `conversation_id`. Counters refreshed immediately after upsert. Deleted by `DELETE /api/sessions/:cid`, which also publishes a `derived/session/{action:"delete"}` broadcast (`backend/llr/API delete session purges traces and projections.md`).
- **Vault refs:** `[[Session upserted per conversation id]]`, `[[Session counters refreshed on session upsert]]`, `[[API delete session purges traces and projections]]`.

### `hook_invocations`

- **Purpose:** Capture pre/post hook events (`github.copilot.hook.start` / `.end`) keyed by `invocation_id` so the start/end pair can arrive separately (`backend/llr/Hook start event derives hook invocation.md`; `backend/llr/Hook end event completes hook invocation.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `invocation_id` | TEXT | no | — | UNIQUE conflict key; `github.copilot.hook.invocation_id`. The vault lists this as the keying column but does not state explicitly whether it is itself the PK or a separate PK exists. |
  | `hook_type` | TEXT | yes | NULL | `github.copilot.hook.type` |
  | `span_pk` | INTEGER | yes | NULL | FK → `spans.span_pk` |
  | `conversation_id` | TEXT | yes | NULL | coalesced on conflict |
  | `start_unix_ns` | INTEGER | yes | NULL | from start event |
  | `end_unix_ns` | INTEGER | yes | NULL | from end event |
  | `duration_ns` | INTEGER | yes | NULL | computed `end - start` only when `start_unix_ns IS NOT NULL` |
  | `agent_run_pk` | INTEGER | yes | NULL | resolved via ancestor walk |
  | `chat_turn_pk` | INTEGER | yes | NULL | resolved via ancestor walk |
  | `tool_call_pk` | INTEGER | yes | NULL | resolved via ancestor walk |

- **Constraints:** UNIQUE `invocation_id`; `conversation_id` coalesced on conflict.
- **Lifecycle:** Upserted twice (once per start event, once per end event). Purged by `delete_session`.
- **Vault refs:** `[[Hook start event derives hook invocation]]`, `[[Hook end event completes hook invocation]]`.

### `skill_invocations`

- **Purpose:** Captures `github.copilot.skill.invoked` span events (`backend/llr/Skill invoked event records skill invocation.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | (PK) | INTEGER | no | AUTOINCREMENT | PK column name not given in vault |
  | `span_pk` | INTEGER | no | — | FK → `spans.span_pk`; part of UNIQUE |
  | `skill_name` | TEXT | no | — | part of UNIQUE |
  | `skill_path` | TEXT | yes | NULL | |
  | `invoked_unix_ns` | INTEGER | no | — | part of UNIQUE; from event time |
  | `conversation_id` | TEXT | yes | NULL | resolved via ancestor walk |
  | `agent_run_pk` | INTEGER | yes | NULL | resolved via ancestor walk |
  | `chat_turn_pk` | INTEGER | yes | NULL | resolved via ancestor walk |

- **Indexes / constraints:** UNIQUE `(span_pk, invoked_unix_ns, skill_name)` — `INSERT ... ON CONFLICT DO NOTHING` so re-delivery does not duplicate (`backend/llr/Skill invoked event records skill invocation.md`).
- **Lifecycle:** Inserted from span events; re-resolved on parent arrival; purged by `delete_session`.
- **Vault refs:** `[[Skill invoked event records skill invocation]]`.

### `context_snapshots`

- **Purpose:** Time-series-style token / context-window snapshots, populated from two distinct sources: chat-span usage attributes and `github.copilot.session.usage_info` events (`backend/llr/Chat token usage attributes create context snapshot.md`; `backend/llr/Usage info event creates context snapshot.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `ctx_pk` | INTEGER | no | AUTOINCREMENT | PK |
  | `span_pk` | INTEGER | yes | NULL | FK → `spans.span_pk`; part of UNIQUE |
  | `conversation_id` | TEXT | yes | NULL | resolved via ancestor walk |
  | `chat_turn_pk` | INTEGER | yes | NULL | resolved via ancestor walk |
  | `captured_ns` | INTEGER | no | — | `end_unix_ns ?? start_unix_ns` for chat_span; event time for usage_info; part of UNIQUE |
  | `token_limit` | INTEGER | yes | NULL | usage_info: `github.copilot.token_limit` |
  | `current_tokens` | INTEGER | yes | NULL | usage_info: `github.copilot.current_tokens` |
  | `messages_length` | INTEGER | yes | NULL | usage_info: `github.copilot.messages_length` |
  | `input_tokens` | INTEGER | yes | NULL | chat_span source |
  | `output_tokens` | INTEGER | yes | NULL | chat_span source |
  | `cache_read_tokens` | INTEGER | yes | NULL | chat_span source |
  | `reasoning_tokens` | INTEGER | yes | NULL | chat_span source |
  | `source` | TEXT | yes | NULL | enum `'chat_span' \| 'usage_info_event'`; part of UNIQUE |

- **Indexes / constraints:** UNIQUE `(span_pk, source, captured_ns)` — fields coalesced on conflict.
- **Lifecycle:** Upserted from chat-span normalization (when any usage attribute is present) and from `usage_info` span events. Listed by `GET /api/sessions/:cid/contexts` ordered `captured_ns ASC`. Purged by `delete_session`.
- **Vault refs:** `[[Chat token usage attributes create context snapshot]]`, `[[Usage info event creates context snapshot]]`, `[[API list session contexts ordered by capture]]`.

### `metric_points`

- **Purpose:** One row per OTLP metric data point (across `gauge`/`sum`/`histogram`/`exponentialHistogram`/`summary`); independently queryable (`backend/llr/Metric data points persisted to metric_points.md`).
- **Columns:**

  | Column | Type | Null | Default | Notes |
  |---|---|---|---|---|
  | `raw_record_id` | INTEGER | no | — | FK → `raw_records.id` |
  | `metric_name` | TEXT | no | — | from `MetricEnvelope.name` |
  | `description` | TEXT | yes | NULL | |
  | `unit` | TEXT | yes | NULL | |
  | `start_unix_ns` | INTEGER | yes | NULL | from `dp.start_time.to_unix_nanos()` |
  | `end_unix_ns` | INTEGER | yes | NULL | from `dp.end_time.to_unix_nanos()` |
  | `attributes_json` | TEXT (JSON) | no | — | `serde_json::to_string(dp.attributes)` |
  | `value_json` | TEXT (JSON) | no | — | full `dp` value JSON, not just numeric |
  | `resource_json` | TEXT (JSON) | yes | NULL | from envelope |
  | `scope_name` | TEXT | yes | NULL | from envelope |
  | `scope_version` | TEXT | yes | NULL | from envelope |

- **Indexes / constraints:** PK column not pinned in the vault (no `metric_point_pk` is mentioned in the cheatsheet column list).
- **Lifecycle:** Insert-only, one row per data point; `delete_session` does not remove these (it deletes only span/projection/session-tagged tables). The vault does not specify a separate purge path.
- **Vault refs:** `[[Metric data points persisted to metric_points]]`, `[[OTLP metrics persists raw and normalizes envelopes]]`, `[[Metric ingest emits raw metric event]]`.

### Logs

`backend/llr/Logs not normalized currently.md` and `backend/__cheatsheets/Normalize Pipeline Cheatsheet.md` confirm that `Envelope::Log(_)` is a no-op for normalization. Log payloads land only in `raw_records` (with `record_type='log'`). There is no `logs` table in the vault.

## Migrations

- Migration files live in `./migrations/` relative to the Rust crate root and are embedded at compile time via `sqlx::migrate!("./migrations")` (`backend/__cheatsheets/DB Module Cheatsheet.md`).
- `db::open` runs all embedded migrations against the pool before returning, on every startup, so the database file is always brought up to the binary's current schema (`backend/llr/DB open runs migrations.md`).
- Tests assert reachability of migration-created tables (`raw_records`, `spans`, `sessions`) rather than version numbers (`backend/__teststubs/DB Module Tests.md`).
- The vault does **not** enumerate individual migration filenames, version numbers, or the SQL inside them.

## Views / Triggers / Pragmas

- **Pragmas (set on every connection):** `journal_mode=WAL`, `synchronous=Normal`, `foreign_keys=ON` (= 1). The DB-tests stub directly probes `PRAGMA journal_mode` → `'wal'` and `PRAGMA foreign_keys` → `1` (`backend/llr/DB open enables WAL and foreign keys.md`; `backend/__teststubs/DB Module Tests.md`; `backend/__cheatsheets/DB Module Cheatsheet.md`).
- **Views:** none mentioned anywhere in the vault.
- **Triggers:** none mentioned anywhere in the vault. All cascading and counter-refresh behaviour described above is performed in application code by the normalizer / `delete_session` handler, not by SQL triggers (`backend/__cheatsheets/Normalize Pipeline Cheatsheet.md`; `backend/__cheatsheets/REST API Cheatsheet.md`).

## Gaps / Unspecified

These are details a real schema would normally pin down, but the vault does not:

- **Migration inventory.** No vault note enumerates the actual migration files, their version numbers, ordering, or the verbatim DDL. Column lists above are reverse-engineered from cheatsheets and LLRs.
- **Column SQL types.** Types (`INTEGER`/`TEXT`, NULL/NOT NULL, defaults) are *inferred* from behaviour (the cheatsheet describes columns by name only). `received_at` in `raw_records` in particular has no documented type or default.
- **Indexes.** Only the explicit UNIQUE constraints (`spans(trace_id, span_id)`, `*.span_pk`, `skill_invocations(span_pk, invoked_unix_ns, skill_name)`, `context_snapshots(span_pk, source, captured_ns)`, plus implicit UNIQUE on `hook_invocations.invocation_id`) are documented. Read-side indexes that would support `list_traces`, `list_spans` filters, `delete_session`'s trace UNION query, etc. are not described.
- **FK ON DELETE/UPDATE actions.** `delete_session` claims that `DELETE FROM spans` "cascades to `span_events` and projection rows tagged by FK", which implies `ON DELETE CASCADE` from `spans` to those tables, but the vault never restates the FK definitions. Whether projection tables (`agent_runs`/`chat_turns`/`tool_calls`/`external_tool_calls`/`hook_invocations`/`skill_invocations`/`context_snapshots`) cascade automatically or are wiped by explicit DELETEs is ambiguous — `backend/llr/API delete session purges traces and projections.md` says the handler issues explicit `DELETE` for each of those tables, suggesting they are *not* cascade-deleted from `spans`.
- **PK on `metric_points` and `skill_invocations`.** The cheatsheet column lists do not name the surrogate PK column for these two tables.
- **`hook_invocations` PK.** The vault treats `invocation_id` as the conflict key but doesn't say whether it is the PK or a separate `INTEGER PRIMARY KEY AUTOINCREMENT` column exists alongside.
- **Retention / vacuum.** No vault note specifies any pruning of `raw_records`, `metric_points`, or `span_events` outside of session-scoped delete; no `VACUUM`/`ANALYZE` or housekeeping is mentioned.
- **Generated columns / CHECK constraints.** None mentioned. The `ingestion_state ∈ {'real','placeholder'}` and `context_snapshots.source ∈ {'chat_span','usage_info_event'}` enums are enforced only in application code per the cheatsheets.
- **Logs persistence.** Beyond "raw only", the vault does not pin down whether a future `logs` table exists in migrations.

(agent_id: backend-db-schema — use write_agent to send follow-up messages)