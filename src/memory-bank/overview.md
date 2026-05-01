# Backend Memory Bank — Overview

> Scope: `src/` — the `ghcp-mon` Rust backend.
> Source of truth: `obsidian/ghcp-mon/backend/` (10 HLRs, 85 LLRs, 16 impl notes; scoped by vault folder rather than tag; 100% test-coverage classification per `obsidian_coverage_report`).
> This file is a context primer. Drill into per-area files for behavior; drill into the vault for normative obligations.

---

## Mission

`ghcp-mon` is a single-binary, local-first **observability sidecar for the GitHub Copilot CLI**. It exposes an **OTLP/HTTP receiver** that ingests traces, metrics, and logs, persists each request body verbatim into **SQLite** (WAL, FK-enforced), and **normalizes** spans into a small set of projections — agent runs, chat turns, tool calls, external tool calls, hook/skill invocations, and context snapshots — keyed off `gen_ai.conversation.id` ancestry. The same binary serves a **JSON REST API**, a **WebSocket live event bus**, and an **embedded SPA**, all from a CLI with `serve` and `replay` subcommands; replay also accepts file-exporter JSONL telemetry. Cross-cutting, every HTTP failure surfaces through a single `AppError` envelope, and the dashboard enriches sessions with human-readable metadata read best-effort from the CLI's per-conversation `workspace.yaml` sidecar.

---

## HLR map

| HLR | Vault path | Memory-bank file(s) |
| --- | --- | --- |
| CLI Entry Point | `backend/hlr/CLI Entry Point.md` | `server-and-lifecycle.md` (with replay touching `ingest.md`) |
| OTLP HTTP Receiver | `backend/hlr/OTLP HTTP Receiver.md` | `ingest.md`, `server-and-lifecycle.md` (router) |
| Telemetry Persistence | `backend/hlr/Telemetry Persistence.md` | `db.md`, `ingest.md` (raw archival), `api.md` (delete cascade) |
| Span Normalization | `backend/hlr/Span Normalization.md` | `normalize.md` |
| Live WebSocket Event Stream | `backend/hlr/Live WebSocket Event Stream.md` | `ws.md`, `normalize.md` (emitters), `api.md` (delete fan-out) |
| Dashboard REST API | `backend/hlr/Dashboard REST API.md` | `api.md`, `server-and-lifecycle.md` (CORS + router) |
| Embedded Dashboard SPA | `backend/hlr/Embedded Dashboard SPA.md` | `server-and-lifecycle.md` |
| File Exporter Replay | `backend/hlr/File Exporter Replay.md` | `ingest.md`, `server-and-lifecycle.md` (CLI) |
| Uniform Error Reporting | `backend/hlr/Uniform Error Reporting.md` | `server-and-lifecycle.md` (consumed everywhere) |
| Local Session Metadata | `backend/hlr/Local Session Metadata.md` | `server-and-lifecycle.md` (`local_session.rs`), `api.md` (enrichment) |

---

## Code area index

| Memory-bank file | Source | LLR count | Primary HLR(s) |
| --- | --- | --- | --- |
| `api.md` | `src/api/mod.rs` | 15 | Dashboard REST API |
| `db.md` | `src/db/mod.rs`, `src/db/dao.rs` | 4 | Telemetry Persistence |
| `ingest.md` | `src/ingest/{mod,otlp,replay}.rs` | 12 | OTLP HTTP Receiver, Telemetry Persistence, File Exporter Replay |
| `normalize.md` | `src/normalize/mod.rs` | 27 | Span Normalization (+ Live WS Event Stream) |
| `ws.md` | `src/ws/{mod,handler}.rs` | 5 | Live WebSocket Event Stream |
| `server-and-lifecycle.md` | `src/main.rs`, `src/server.rs`, `src/lib.rs`, `src/local_session.rs`, `src/static_assets.rs`, `src/model.rs`, `src/error.rs` | 22 | CLI Entry Point, Uniform Error Reporting, Embedded Dashboard SPA, Local Session Metadata |
| **Total** | | **85** | |

LLR totals reconcile to **85 LLRs across 16 impl notes** (matches vault). `src/lib.rs` is intentionally LLR-free per its impl note ("pure module declarations").

---

## Glossary

- **Envelope** — internal `Span | Metric | Log` shape (`src/model.rs`) the ingest layer converts OTLP/replay payloads into before normalize.
- **Raw record** — row in `raw_records` storing the verbatim request body or replay line, keyed by `record_type` (`otlp-traces`, `otlp-metrics`, `otlp-logs`).
- **Normalization / projection** — upserting per-kind tables (`agent_runs`, `chat_turns`, `tool_calls`, `external_tool_calls`, `hook_invocations`, `skill_invocations`, `context_snapshots`) from spans.
- **Conversation id** — `gen_ai.conversation.id` attribute; the user-visible session key. **Effective conversation id** is the value inherited via ancestor walk when the span itself omits it.
- **Session** — `sessions` row keyed by `conversation_id`; aggregates first/last-seen, latest model, and projection counters.
- **Placeholder span** — `ingestion_state='placeholder'` row inserted to satisfy a child's `parent_span_id` before the parent arrives. **Upgrade** = replacement of a placeholder by its real span on later ingest.
- **Ingestion state** — `'real' | 'placeholder'` discriminator on `spans`; once real, never demoted.
- **Ancestor walk / forward resolve** — bidirectional re-resolution of projection foreign-key pointers (`parent_*_pk`, `conversation_id`) on each span upsert; depths 64 (up) and 128 (down).
- **Broadcaster / EventMsg** — `tokio::sync::broadcast` fan-out channel and the JSON envelope (`kind`, `entity`, `payload`) it carries to WS clients.
- **Span PK (`span_pk`)** — DB-assigned auto-increment primary key on `spans`, distinct from the OTLP `(trace_id, span_id)` natural key.
- **Kind class** — `SpanKindClass` enum (`InvokeAgent | Chat | ExecuteTool | ExternalTool | Other`) derived purely from span name.
- **AppState** — shared handler context (DB pool, broadcaster, session-state-dir override) built in `main.rs` and routed through `server.rs`.
- **AppError** — single error enum (`BadRequest`, `NotFound`, `NotImplemented`, `Sqlx`, `Migrate`, `Json`, `Io`, `Other`) with deterministic status mapping and JSON body shape.
- **`workspace.yaml` sidecar** — Copilot CLI's per-conversation file at `$session-state-dir/<cid>/workspace.yaml` carrying `local_name`, `user_named`, `cwd`, `branch`.
- **File-exporter format** — JSON-lines telemetry archive, one externally-tagged envelope (`{"type":"span"|"metric"|"log", …}`) per non-blank line.
- **Inline replay** — `replay --inline` builds `AppState` in-process and skips HTTP; the non-inline path POSTs to `/api/replay` of a running `serve`.

---

## Cross-cutting concerns

The vault uses no `concern/*` namespace; cross-cutting is expressed through `domain/*` HLR/LLR tags.

- **`domain/error`** — `error.rs`, but every handler in `api/`, `ingest/`, `ws/`, and `server.rs` returns through it.
- **`domain/local-session`** — `local_session.rs` (resolver + reader) ↔ `api/mod.rs` (enrichment) ↔ `main.rs` (CLI flag plumbing).
- **`domain/replay`** — `ingest/replay.rs` (HTTP endpoint) ↔ `ingest/mod.rs` (file reader, parser) ↔ `model.rs` (envelope tag) ↔ `main.rs` (subcommand, inline vs non-inline).
- **`domain/ws`** — produced by `normalize/mod.rs` (six emitters) and `api/mod.rs` (delete fan-out), consumed by `ws/handler.rs`, mounted via `server.rs`.
- **`domain/normalize` ∩ `domain/ws`** — `Span normalize emits span and trace events` is dual-tagged and dual-parented.
- **`domain/cli` ∩ `domain/replay`** — `Replay inline mode ingests in-process` and `Replay non-inline posts to running server` straddle CLI Entry Point and File Exporter Replay.
- **`domain/otlp` ∩ `domain/db`** — `OTLP traces/metrics persists raw and normalizes envelopes` and `Raw request body persisted verbatim per OTLP request` straddle the receiver and persistence HLRs.
- **`domain/api` ∩ multiple HLRs** — `API delete session purges traces and projections` is parented by **four** HLRs (Dashboard REST API, Telemetry Persistence, Live WebSocket Event Stream, Uniform Error Reporting) — the most cross-cutting LLR in the backend.
- **`AppState` plumbing** — implicit cross-cut: `db::Pool`, `Broadcaster`, and `session_state_dir_override` thread from `main.rs` through `server.rs` into every handler.

---

## Integration seams (high-leverage spots to read first)

1. **`AppState` construction in `main.rs`** — the seam every other layer plugs into.
2. **`server.rs` route table** — only place OTLP, REST, WS, and SPA layers meet.
3. **`ingest_envelope` (in `ingest/mod.rs`)** — single chokepoint feeding both OTLP and replay paths into `db/` then `normalize/`.
4. **`Broadcaster::send` call sites in `normalize/mod.rs` and `api/mod.rs`** — the live event surface.
5. **`local_session::resolve_session_state_dir` + `read_workspace_yaml`** — the only FS read outside the SQLite path; security-sensitive (path-traversal LLR).
6. **`AppError` `From` impls** — every other layer's failure modes funnel through here.

---

## Coverage notes (read before trusting the bank)

- **Test coverage:** 85/85 LLRs classified `COVERED` per `obsidian_coverage_report scopes=["backend"]`. No `BLOCKED`, `MISSING-PREP`, or `READY-FOR-TEST` entries.
- **Orphans:** none. All LLRs reach an HLR via explicit `Derived from` links.
- **Multi-parent LLRs (legitimate cross-cuts):**
  - `API delete session purges traces and projections` → 4 HLRs (Dashboard REST API, Telemetry Persistence, Live WS Event Stream, Uniform Error Reporting).
  - `OTLP traces/metrics persists raw and normalizes envelopes` → 2 HLRs each.
  - `Span normalize emits span and trace events` → 2 HLRs.
  - `Replay inline mode ingests in-process` → 2 HLRs.
  - `API list sessions enriched with local workspace metadata`, `API session detail enriched with local workspace metadata` → 2 HLRs each.
  - `Raw request body persisted verbatim per OTLP request` → 2 HLRs.
- **LLR-as-parent edge:** `Session upsert emits derived session event` declares a `See also` cross-link to a sibling LLR (`API delete session purges traces and projections`). This is documentation cross-reference (the index of legal `derived/session` `action` values), not a true derivation; safe to ignore for parentage but preserved here in the WS/normalize → API delete narrative.
- **Repo files with no vault coverage:**
  - `src/lib.rs` — intentionally empty per its impl note ("pure module declarations"); not a gap.
  - Outside `src/`: `build.rs`, `migrations/`, `web/` (frontend, see `web/memory-bank/`), `tests/`, `dist/`, `data/`, `Containerfile`, `BUILDING.md`, `doc/`, `reference/` — none have backend impl notes; the SQL schema in `migrations/` is implicitly referenced by `db` and `normalize` LLRs (table names appear in obligation text) but is not covered by its own LLR.
- **Integration with frontend:** the contract surface is the JSON shapes emitted from `src/api/mod.rs` and `src/ws/*` and consumed by `web/src/api/types.ts` (`API types mirror backend span-canonical model`). No requirement-graph cross-references; the seam is implicit.
