# server-and-lifecycle — Memory Bank (backend)

## Purpose
This is the **wiring layer** of the backend — the thin integration seam where every other module is composed into a runnable binary. It owns CLI parsing (`clap`), construction of the shared `AppState`, the `tracing_subscriber` registry, the two axum routers (OTLP + API/WS/SPA), the permissive CORS layer, the 64 MiB OTLP body limit, the uniform `AppError → JSON` envelope, the embedded-SPA static handler, and the filesystem sidecar reader for `workspace.yaml` files written by the Copilot CLI. Nothing in this area implements business logic; everything here is the place where ingest, persistence, projection, REST, WebSocket, and UI bytes meet — and the place where their failure modes are normalized into a single error shape.

## Key behaviors

### main.rs — CLI + AppState
- The binary exposes exactly two subcommands, `serve` and `replay`. `serve` starts the OTLP receiver, REST API, and WebSocket; `replay` replays a JSON-lines telemetry file. `CLI defines serve and replay subcommands`
- A global `--db <path>` option selects the SQLite database; when omitted it defaults to `./data/ghcp-mon.db`. `CLI db option default path`
- A global `--session-state-dir <PATH>` flag (clap `global = true`, optional) overrides the base directory for `workspace.yaml` sidecars. The parsed value is threaded into `AppState.session_state_dir_override` (an `Arc<Option<PathBuf>>`) for both `Serve` and `Replay --inline`; API handlers resolve the effective directory by passing the override into `local_session::resolve_session_state_dir`. `CLI session state dir flag overrides default`
- On startup the binary initializes a `tracing_subscriber` registry honoring `RUST_LOG`, falling back to `info,sqlx=warn,tower_http=warn,hyper=warn` when the env var is unset. `CLI initializes tracing subscriber`
- `serve` binds two TCP listeners — OTLP at `--otlp-addr` (default `127.0.0.1:4318`) and REST API + WebSocket at `--api-addr` (default `127.0.0.1:4319`) — and runs both concurrently until either fails. `Serve binds OTLP and API listeners`
- `replay --inline` opens the database, builds an in-process `AppState`, calls `ingest_jsonl_file` against the supplied path, and prints the number of envelopes ingested. No HTTP server is contacted. `Replay inline mode ingests in-process`
- `replay` without `--inline` canonicalizes the supplied path, POSTs `{"path": "<canonical path>"}` to `<server>/api/replay` (default `<server>` = `http://127.0.0.1:4319`, any trailing slash stripped), and prints status code and response body. `Replay non-inline posts to running server`

### server.rs — router + middleware
- The OTLP router exposes three POST routes — `/v1/traces`, `/v1/metrics`, `/v1/logs` — handled by `ingest::otlp::traces`, `ingest::otlp::metrics`, and `ingest::otlp::logs`. The path layout matches the OTLP/HTTP convention so unmodified OTel exporters work without configuration. `OTLP router exposes traces metrics logs endpoints`
- The OTLP router applies an axum `DefaultBodyLimit` of 64 MiB (`64 * 1024 * 1024`). Axum's 2 MiB default is too small for batched OTLP traces from real CLI sessions. `OTLP body limit 64 MiB`
- The API router mounts the full dashboard URL surface — GETs `/api/healthz`, `/api/sessions`, `/api/sessions/:cid`, `/api/sessions/:cid/span-tree`, `/api/sessions/:cid/contexts`, `/api/sessions/:cid/registries`, `/api/spans`, `/api/spans/:trace_id/:span_id`, `/api/traces`, `/api/traces/:trace_id`, `/api/raw`, `/ws/events`; POST `/api/replay`; DELETE `/api/sessions/:cid`; and a fallback that serves the embedded SPA. `API router exposes session and span endpoints`
- The API router installs a CORS layer that allows any origin, any method, and any headers — intentional for local-first usage where the dashboard frontend may be served from any localhost port. `API allows any origin via CORS`

### error.rs — AppError envelope
- `AppError → Response` mapping: `BadRequest → 400`, `NotFound → 404`, `NotImplemented → 501`, all other variants (`Sqlx`, `Migrate`, `Json`, `Io`, `Other`) → `500`. One mapping table keeps client error handling deterministic. `AppError maps variants to status codes`
- The HTTP body for every `AppError` is `{"error": "<message>"}`. `BadRequest` and `NotImplemented` carry the supplied string; `NotFound` carries `"not found"`; other variants use `Display`. The dashboard expects this uniform JSON shape regardless of failure origin. `AppError JSON body contains error message`
- `AppError` provides `From` impls for `sqlx::Error`, `sqlx::migrate::MigrateError`, `serde_json::Error`, and `std::io::Error`, so handlers can use `?` directly on these types without per-call `.map_err(...)`. `AppError converts from sqlx serde io migrate`

### local_session.rs — sidecar reader
- `local_session::resolve_session_state_dir(override_dir)` resolves the base directory in strict precedence: (1) `Some(path)` from `--session-state-dir` wins, (2) else `$COPILOT_SESSION_STATE_DIR` if set and non-empty, (3) else `$HOME/.copilot/session-state` if `$HOME` is set and non-empty, (4) else `None`. `default_session_state_dir()` is `resolve_session_state_dir(None)`. `Local session state dir resolved from flag env or home`
- `local_session::read_workspace_yaml(base, cid)` attempts to read `<base>/<cid>/workspace.yaml` and parse it as `WorkspaceYaml { id, name, user_named, summary, cwd, git_root, branch, created_at, updated_at }` — all fields optional. Returns `Some(WorkspaceYaml)` on success and `None` on any I/O or parse error. The sidecar is owned by the Copilot CLI, so the dashboard never fails when metadata is absent. `Local session workspace yaml best effort read`
- `local_session::read_workspace_yaml(base, cid)` returns `None` **without touching the filesystem** when `cid` is empty or contains any of the substrings `/`, `\\`, or `..`. Defense-in-depth: `cid` arrives from URL paths, so the lookup must never escape `base`. `Local session workspace yaml rejects path traversal`

### static_assets.rs — embedded SPA
- `static_handler` strips the leading `/` from the request URI's path (mapping the empty path to `index.html`) and uses the result as a key into the embedded `web/dist/` asset bundle. On a hit it returns HTTP 200 with the asset bytes. `Static handler serves embedded asset by path`
- When no asset matches, `static_handler` falls back to serving `index.html` so the SPA's client-side router can handle the URL. Deep links like `/sessions/abc` must not 404 just because no static file matches. `Static handler SPA fallback to index html`
- When no asset matches **and** `index.html` is also absent from the bundle, `static_handler` returns HTTP 404 with body `"not found"`. A build that ships without the SPA must surface that fact rather than serving empty bodies. `Static handler returns 404 when index missing`
- The `Content-Type` header is set from the asset path's extension via `mime_guess`, defaulting to `application/octet-stream` when no guess is available. Browsers need the right MIME type to load JS/CSS/fonts/images; binary fallback prevents misrendering. `Static handler sets content type from extension`

### model.rs — shared types
- `SpanKindClass::from_name(name)` classifies a span name into the projection-target class: `InvokeAgent` if `name == "invoke_agent"` or starts with `"invoke_agent "`; `Chat` if it starts with `"chat"`; `ExecuteTool` if it starts with `"execute_tool"`; `ExternalTool` if it starts with `"external_tool"`; `Other` otherwise. Span name is the **sole input** that determines which projection table is updated. `Span name classified into kind class`
- (Also co-sourced from `model.rs`, but counted under ingest:) `parse_file_exporter_line` deserializes a JSON object whose `type` field is `"span"`, `"metric"`, or `"log"` into the corresponding `Envelope::Span`/`Envelope::Metric`/`Envelope::Log` variant, returning `AppError::BadRequest` on parse failure. `Replay parser tags envelopes by type`

### lib.rs — module declarations only (no LLRs)
- By design, `src/lib.rs` is pure module declarations and re-exports — no behavior, no LLRs. The impl note classifies it explicitly as "pure module declarations." Mentioned here only for completeness; nothing to assert.

## Public surface
- **CLI subcommands:** `serve`, `replay`
- **CLI flags:**
  - `--db <PATH>` (global, default `./data/ghcp-mon.db`)
  - `--session-state-dir <PATH>` (global, optional; threads into `AppState.session_state_dir_override`)
  - `--otlp-addr <ADDR>` (serve, default `127.0.0.1:4318`)
  - `--api-addr <ADDR>` (serve, default `127.0.0.1:4319`)
  - `--inline` (replay; in-process ingest, no HTTP)
  - replay non-inline targets `<server>/api/replay`, default server `http://127.0.0.1:4319`
- **Shared state:** `AppState { db: Pool, broadcaster: Broadcaster, session_state_dir_override: Arc<Option<PathBuf>> }`
- **Router mounts:**
  - OTLP: `POST /v1/traces`, `POST /v1/metrics`, `POST /v1/logs` (64 MiB body limit)
  - API GETs: `/api/healthz`, `/api/sessions`, `/api/sessions/:cid`, `/api/sessions/:cid/span-tree`, `/api/sessions/:cid/contexts`, `/api/sessions/:cid/registries`, `/api/spans`, `/api/spans/:trace_id/:span_id`, `/api/traces`, `/api/traces/:trace_id`, `/api/raw`, `/ws/events`
  - API POST: `/api/replay`
  - API DELETE: `/api/sessions/:cid`
  - Fallback: embedded SPA via `static_handler`
- **Error type:** `AppError` enum (`BadRequest`, `NotFound`, `NotImplemented`, `Sqlx`, `Migrate`, `Json`, `Io`, `Other`) with `IntoResponse` and `From` impls for `sqlx::Error`, `sqlx::migrate::MigrateError`, `serde_json::Error`, `std::io::Error`
- **Local-session API:**
  - `local_session::resolve_session_state_dir(override_dir: Option<&Path>) -> Option<PathBuf>`
  - `local_session::default_session_state_dir() -> Option<PathBuf>` (≡ `resolve_session_state_dir(None)`)
  - `local_session::read_workspace_yaml(base, cid) -> Option<WorkspaceYaml>`
  - `WorkspaceYaml { id, name, user_named, summary, cwd, git_root, branch, created_at, updated_at }` — all optional
- **Static handler:** `static_handler` — embedded `web/dist/` bundle, SPA fallback, MIME via `mime_guess`
- **Shared model types:** `model::SpanKindClass` (`InvokeAgent`, `Chat`, `ExecuteTool`, `ExternalTool`, `Other`); externally-tagged `Envelope` enum (`Span` / `Metric` / `Log`) consumed by the file-exporter replay parser

## Invariants & constraints
- **AppError → status codes:** `BadRequest=400`, `NotFound=404`, `NotImplemented=501`, all others (`Sqlx`/`Migrate`/`Json`/`Io`/`Other`) `=500`. One mapping table — no per-handler exceptions.
- **AppError body shape:** `{"error":"<msg>"}` JSON for every variant. `BadRequest` / `NotImplemented` carry the supplied string; `NotFound` always carries `"not found"`; other variants use `Display`.
- **AppError From impls:** blanket conversions for `sqlx::Error`, `sqlx::migrate::MigrateError`, `serde_json::Error`, `std::io::Error`. Handlers should use `?` rather than `.map_err`.
- **OTLP body limit:** exactly `64 * 1024 * 1024` bytes via axum `DefaultBodyLimit` on the OTLP router. The default 2 MiB is insufficient for batched real-world traces.
- **CORS:** API router permits any origin, any method, any headers. Intentional and local-first; do not tighten without revisiting the HLR.
- **Listener lifetime:** `serve` binds OTLP and API listeners concurrently and runs **until either fails** — failure of one aborts the binary; both must stay healthy together.
- **Workspace.yaml read is best-effort:** missing file or parse error → `None`, never a propagated error. The dashboard must never fail because a session has no metadata yet.
- **Path-traversal guard:** empty `cid`, or `cid` containing `/`, `\\`, or `..`, returns `None` **before any filesystem access**. Defense-in-depth on URL-derived paths.
- **Session state dir precedence:** flag (`--session-state-dir`) → `$COPILOT_SESSION_STATE_DIR` → `$HOME/.copilot/session-state` → `None`. Explicit flag always wins so per-invocation overrides are deterministic.
- **`--session-state-dir` threading:** stored on `AppState` as `Arc<Option<PathBuf>>` for both `Serve` and `Replay --inline`; API handlers always resolve the effective directory by calling `resolve_session_state_dir` with that override (never read env directly).
- **SPA fallback:** any unknown path serves `index.html`; if the bundle has no `index.html`, return 404 `"not found"`. No empty bodies.
- **Static content-type:** derived from the asset extension via `mime_guess`; `application/octet-stream` is the only fallback.
- **Tracing default filter:** `info,sqlx=warn,tower_http=warn,hyper=warn` when `RUST_LOG` is unset.
- **DB default path:** `./data/ghcp-mon.db` (local-first, no required configuration).
- **Default ports:** OTLP `127.0.0.1:4318`, API `127.0.0.1:4319` (separable so operators can apply different network policies).
- **Replay non-inline canonicalization:** the path is canonicalized client-side before POST; `<server>` strips any trailing slash; default `<server>` is `http://127.0.0.1:4319`.
- **Replay inline isolation:** does not contact any HTTP server; uses an in-process `AppState`.
- **Span classification:** `SpanKindClass::from_name` is the **only** function that picks the projection target. Any new span family must extend it explicitly.
- **Replay envelope tagging:** the file-exporter format uses an externally-tagged `type` discriminant; unrecognized / unparseable lines yield `AppError::BadRequest`.

## Integration seams (the big picture)
1. **`AppState` construction in `main.rs`** is the single point where `db::Pool`, `Broadcaster`, and `session_state_dir_override` are assembled. Every handler in the binary takes `AppState` (or a clone) — change its shape here and you change every handler's signature.
2. **`server.rs` route table** is the only place where the OTLP receiver, REST API, WebSocket upgrade, and SPA fallback layers meet. URL stability for clients is owned here, as is the OTLP body limit and the API CORS layer.
3. **`local_session::*`** is the only filesystem read path outside SQLite. It is security-sensitive (URL-derived `cid`) and is gated by both a path-traversal guard and a best-effort I/O contract — both are invariants of the module, not handler-level concerns.
4. **`AppError` From impls** funnel every other layer's failure modes (sqlx, serde, io, migrate) into the uniform JSON envelope. Anywhere the binary touches an error, it eventually flows through `error.rs`.
5. **`model.rs`** holds two cross-area shared types: the externally-tagged `Envelope` enum that the replay parser produces and the ingest pipeline consumes, and `SpanKindClass` that span normalization uses to pick a projection target. Both are pure data; they connect ingest, replay, and projection without coupling them directly.

## Dependencies
- **Inbound from this area:** `db::Pool` (built in `db/`) and `Broadcaster` (from `ws/`) are constructed in `main.rs` and stored on `AppState`.
- **Outbound from this area:** `server.rs` mounts handlers from `api/` (REST), `ingest/otlp.rs` (OTLP receiver), `ingest/replay.rs` (replay endpoint), `ws/` (WebSocket upgrade), and `static_assets.rs` (SPA fallback).
- **Error contract:** every handler in the binary returns `Result<_, AppError>`, so every other module depends on `error.rs`.
- **CLI ↔ replay:** `replay --inline` calls `ingest_jsonl_file` directly; non-inline mode is a thin HTTP client against `/api/replay`.
- **Tracing:** initialized once in `main.rs`; no module re-initializes the subscriber.

## Repo files outside vault scope (related to lifecycle)
- `build.rs` — embeds the SPA assets (`web/dist/`) into the binary at build time. No LLR; consumed by `static_assets.rs`.
- `migrations/` — SQL schema migrations. No LLR in this area; referenced by name from the `db/` and normalize requirements.
- `Containerfile` — container image build recipe. Packaging only.
- `BUILDING.md` — developer build instructions. Documentation only.
- `doc/`, `reference/` — supporting documentation and reference material; not tied to LLRs.
- `tests/` — integration test scaffolding (the per-LLR test cases live as `*_Tests` notes in the vault).
- `dist/` — build output / packaging artifacts.
- `web/` — the SPA source tree; its compiled output is what `build.rs` embeds.

## Where to read for detail

### HLRs (4)
- `backend/hlr/CLI Entry Point.md` — operator-facing CLI, global config sharing.
- `backend/hlr/Uniform Error Reporting.md` — single application error type, uniform JSON shape.
- `backend/hlr/Embedded Dashboard SPA.md` — single-binary serve of API + UI from embedded bundle.
- `backend/hlr/Local Session Metadata.md` — best-effort sidecar reader for `workspace.yaml`.
- (Routing layer also serves two HLRs from other areas: `OTLP HTTP Receiver` and `Dashboard REST API`.)

### LLRs (22)
**main.rs (CLI + AppState) — 7**
- `backend/llr/CLI defines serve and replay subcommands.md`
- `backend/llr/CLI db option default path.md`
- `backend/llr/CLI session state dir flag overrides default.md`
- `backend/llr/CLI initializes tracing subscriber.md`
- `backend/llr/Serve binds OTLP and API listeners.md`
- `backend/llr/Replay inline mode ingests in-process.md`
- `backend/llr/Replay non-inline posts to running server.md`

**server.rs (router + middleware) — 4**
- `backend/llr/OTLP router exposes traces metrics logs endpoints.md`
- `backend/llr/OTLP body limit 64 MiB.md`
- `backend/llr/API router exposes session and span endpoints.md`
- `backend/llr/API allows any origin via CORS.md`

**error.rs (AppError envelope) — 3**
- `backend/llr/AppError maps variants to status codes.md`
- `backend/llr/AppError JSON body contains error message.md`
- `backend/llr/AppError converts from sqlx serde io migrate.md`

**local_session.rs (sidecar reader) — 3**
- `backend/llr/Local session state dir resolved from flag env or home.md`
- `backend/llr/Local session workspace yaml best effort read.md`
- `backend/llr/Local session workspace yaml rejects path traversal.md`

**static_assets.rs (embedded SPA) — 4**
- `backend/llr/Static handler serves embedded asset by path.md`
- `backend/llr/Static handler SPA fallback to index html.md`
- `backend/llr/Static handler returns 404 when index missing.md`
- `backend/llr/Static handler sets content type from extension.md`

**model.rs (shared types) — 1 unique (+1 co-sourced)**
- `backend/llr/Span name classified into kind class.md` (unique)
- `backend/llr/Replay parser tags envelopes by type.md` (co-sourced from model.rs; counted under ingest)

**lib.rs — 0**
- Module declarations only; no LLRs by design.

### Source files (7)
- `src/main.rs` — entry point: clap parsing, tracing init, `AppState` construction, `serve` listener wiring, `replay` (inline + non-inline) drivers.
- `src/server.rs` — axum router assembly: OTLP routes + 64 MiB body limit, API routes + permissive CORS, SPA fallback mount.
- `src/error.rs` — `AppError` enum, `IntoResponse` mapping, JSON envelope, `From` impls.
- `src/local_session.rs` — session-state-dir precedence resolver and best-effort `workspace.yaml` reader with path-traversal guard.
- `src/static_assets.rs` — `static_handler` for embedded `web/dist/` bundle: lookup, SPA fallback, 404 when index missing, MIME via `mime_guess`.
- `src/model.rs` — shared envelope tag enum (`Envelope::Span`/`Metric`/`Log`) and `SpanKindClass` classifier.
- `src/lib.rs` — pure module declarations and re-exports; no behavior, no LLRs.