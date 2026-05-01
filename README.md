# ghcp-mon

Local-first telemetry collector + realtime dashboard for the GitHub Copilot CLI's OpenTelemetry export.

- **Backend** (`./`): Rust + axum + sqlx (SQLite WAL). OTLP/HTTP receiver, normalization pipeline, REST API, WebSocket fanout.
- **Frontend** (`./web/`): React + TypeScript + Vite. Multi-column dashboard with 6 scenario types (live sessions, spans, tool detail inspector, input breakdown, file touches, raw record browser).

See `./plan.md` for the project plan, `./supplemental.md` for the supplemental requirements, and `./docs/api.md` for the full HTTP API contract.

## Listeners

| Listener  | Default          | Purpose                                                              |
|-----------|------------------|----------------------------------------------------------------------|
| OTLP      | `127.0.0.1:4318` | OTLP/HTTP receiver: `POST /v1/{traces,metrics,logs}` (JSON only — protobuf returns 501) |
| API + WS  | `127.0.0.1:4319` | Dashboard REST + WebSocket fanout (`/api/...`, `/ws/events`)         |
| Vite dev  | `127.0.0.1:5173` | Frontend dev server                                                  |

Override with `--otlp-addr` / `--api-addr` on `ghcp-mon serve`. The global `--session-state-dir <PATH>` flag overrides the default location (`$COPILOT_SESSION_STATE_DIR` env var, then `$HOME/.copilot/session-state`) used to read per-conversation `workspace.yaml` sidecar metadata.

## Run

### Backend

```bash
cargo build
cargo run -- serve                       # boots both listeners
cargo run -- serve --db /tmp/ghcp.db     # custom DB path
```

The DB defaults to `./data/ghcp-mon.db` (WAL mode, foreign keys on, single migration `0001_init.sql`).

### Frontend

```bash
cd web
npm install
npm run dev                              # vite at :5173
```

Open <http://127.0.0.1:5173>. The default workspace seeds four columns: Sessions | Traces | Tool detail | Input breakdown. Use the column header buttons to add, remove, reorder, or change scenario type.

### Capture from Copilot CLI

To pipe live Copilot CLI telemetry into the running server:

```bash
OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318 \
OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true \
OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental \
copilot
```

`OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true` is what makes prompt content, tool args, and shell stdio actually appear in the dashboard's content-aware views. Without it the inspector renders a muted "no content captured" line.

## Layout

```
.
├── Cargo.toml
├── src/                    # axum server, normalization, ingest, ws
├── migrations/0001_init.sql
├── docs/api.md             # full REST + WS contract (TypeScript types)
├── data/                   # runtime SQLite DB (gitignored)
├── reference/              # spec material + sample telemetry (read-only)
├── plan.md                 # project plan
├── supplemental.md         # supplemental requirements
└── web/                    # React + Vite dashboard
    ├── src/api/            # typed client + ws subscription
    ├── src/state/          # Zustand workspace + live-event store
    ├── src/scenarios/      # 6 scenario column components
    └── src/components/     # Column, Inspector, MessageView, JsonView, ContextGrowthWidget, …
```

## Known limitations

- OTLP/protobuf returns `501 Not Implemented`. JSON path is implemented.
- `/v1/logs` is raw-only; not yet derived into a normalized `logs` table.
- Nested subagent recursion in the unified inspector renders one level deep (a follow-up `/api/agent-runs/:pk/detail` endpoint would unblock arbitrary depth).
- Content-aware rendering (prompts / shell stdio / reasoning) requires `OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true`. When content is absent, the inspector renders the empty-state line.
- WebSocket fanout is in-memory only — newly-connected clients see only subsequent events. Use REST for backfill.
