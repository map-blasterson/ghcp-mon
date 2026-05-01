# Frontend Memory Bank — Overview

> Scope: `web/src/` — the `ghcp-mon` browser dashboard.
> Source of truth: `obsidian/ghcp-mon/frontend/` (10 HLRs, 89 LLRs, 25 impl→source bindings, scoped by vault folder rather than tag).
> This file is a context primer. Drill into per-area files for behavior; drill into the vault for normative obligations.

---

## Mission

`ghcp-mon`'s frontend is a single-page **React + Vite + TanStack-Query** dashboard that observes the local backend's view of GitHub Copilot conversations in real time. The user assembles a workspace of side-by-side **scenario columns** — Live Sessions, Trace/Span Explorer, Chat Detail, Tool Detail, File Touches, Raw Records — whose ordering, widths, titles, and presence persist across reloads. Each column reads the backend through one typed REST client mirroring the server's span-canonical model, and stays live via a single WebSocket bus that fans envelopes out to per-`(kind, entity)` ring-buffer subscribers. On top of that substrate the dashboard renders specialized inspectors — a chat-content byte tree, tool-call detail with per-tool layouts, a session file-touch tree, a per-turn context-growth chart, and a raw-record browser — letting the user inspect exactly what is being sent into and pulled out of the model.

---

## HLR map

Per-scope HLR → child LLR groups → memory-bank file that covers it.

| HLR | Vault path | Memory-bank file(s) |
| --- | --- | --- |
| Workspace Layout | `frontend/hlr/Workspace Layout.md` | `app-shell.md`, `components.md`, `state.md`, `scenarios.md` (registry) |
| REST API Client | `frontend/hlr/REST API Client.md` | `api.md` |
| Live WebSocket Subscription | `frontend/hlr/Live WebSocket Subscription.md` | `api.md` (`ws.ts`), `state.md` (`live.ts`) |
| Live Session Browser | `frontend/hlr/Live Session Browser.md` | `scenarios.md` |
| Trace and Span Explorer | `frontend/hlr/Trace and Span Explorer.md` | `scenarios.md`, `components.md` (Inspector, KindBadge) |
| Chat detail | `frontend/hlr/Chat detail.md` | `scenarios.md`, `components.md` (content.ts, MessageView) |
| Tool Call Inspector | `frontend/hlr/Tool Call Inspector.md` | `scenarios.md`, `components.md` (CodeBlock, JsonView) |
| File Touch Tree | `frontend/hlr/File Touch Tree.md` | `scenarios.md` |
| Raw Record Browser | `frontend/hlr/Raw Record Browser.md` | `scenarios.md` |
| Context Growth Widget | `frontend/hlr/Context Growth Widget.md` | `components.md`, `state.md` (`hover.ts`) |

Every HLR appears in at least one per-area file. `Workspace Layout` is the cross-cutting shell HLR — it shows up in four area files because it spans the whole UI.

---

## Code area index

| Memory-bank file | Code area | LLR count | Primary HLRs | Key source files |
| --- | --- | --- | --- | --- |
| `app-shell.md` | (root) `web/src/App.tsx`, `web/src/main.tsx` | 4 | Workspace Layout | `App.tsx`, `main.tsx` |
| `api.md` | `web/src/api/` | 12 | REST API Client, Live WS Subscription | `client.ts`, `types.ts`, `ws.ts` |
| `components.md` | `web/src/components/` | 30 | Workspace Layout, Chat detail, Tool Call Inspector, Trace/Span Explorer, Context Growth Widget | `Workspace.tsx`, `Column.tsx`, `ColumnHeader.tsx`, `Inspector.tsx`, `KindBadge.tsx`, `MessageView.tsx`, `JsonView.tsx`, `CodeBlock.tsx`, `content.ts`, `ContextGrowthWidget.tsx` |
| `scenarios.md` | `web/src/scenarios/` | 36 | Live Session Browser, Trace/Span Explorer, Chat detail, Tool Call Inspector, File Touch Tree, Raw Record Browser, Workspace Layout (registry) | `LiveSessions.tsx`, `Spans.tsx`, `ChatDetail.tsx`, `ToolDetail.tsx`, `FileTouches.tsx`, `RawBrowser.tsx`, `index.ts` |
| `state.md` | `web/src/state/` | 7 | Workspace Layout, Live WS Subscription, Context Growth Widget | `workspace.ts`, `live.ts`, `hover.ts` |

LLR totals reconcile to **89 LLRs across 25 impl notes** (matches vault).

---

## Glossary

- **Workspace** — the persisted multi-column dashboard root.
- **Column** — one slot in the workspace; holds one scenario instance with weight + title.
- **Scenario** — a typed pluggable view (LiveSessions, Spans, ChatDetail, ToolDetail, FileTouches, RawBrowser) registered in `scenarios/index.ts`.
- **Scenario type** — discriminator string used by the registry and persistence; obsolete values are dropped on load.
- **Span** — backend canonical record; the trace explorer renders the parent/child tree.
- **Span kind / kind class** — categorization that gates selection routing between columns.
- **Chat span** — the GenAI request span carrying content attributes; sole input to Chat Detail.
- **Content attribute** — one of system / tools / input / output, parsed from raw JSON-or-object values.
- **Turn** — one chat round (input/output/reasoning bytes); x-axis of the context-growth chart.
- **Tool call projection** — backend's normalized view of an `execute_tool` / `external_tool` span; required by ToolDetail.
- **File touch** — any view/edit/create tool call, aggregated into per-path counts.
- **Envelope** — one parsed WS message flowing through the bus; subscribers keyed by `(kind, entity)`.
- **Live feed / ring buffer** — per-key bounded queue (cap 500) feeding subscribers.
- **WS bus** — singleton lazily-started WebSocket connection with exponential-backoff reconnect.
- **Invalidation** — pattern where a live envelope triggers TanStack-Query invalidation in a scenario.

---

## Cross-cutting concerns

The vault uses no `concern/*` namespace; cross-cutting axes surface via `domain/*` HLR tags:

- **`domain/live-events`** — `api/ws.ts`, `state/live.ts`, and every scenario via per-HLR "live invalidation" LLRs (5 of them).
- **`domain/api-client`** — `api/*`; consumed by every scenario.
- **`domain/workspace`** — `state/workspace.ts`, `components/Workspace.tsx` + `Column*`, app shell, scenario registry.
- **Cross-column hover** (untagged, but cross-cutting) — `state/hover.ts` ↔ `components/ContextGrowthWidget.tsx` ↔ `scenarios/Spans.tsx`.

---

## Coverage notes (read before trusting the bank)

- **No orphan LLRs.** Every frontend LLR resolves to an HLR via `Derived from` links. (Earlier discovery transiently flagged three ToolDetail LLRs as orphaned because the HLR's `## Derived LLRs` list did not enumerate them; the underlying `Derived from` edges existed and the graph is consistent.)
- **One LLR with an LLR-as-parent edge:** `Workspace lays out columns by weight with ResizeObserver` lists both the HLR and a sibling LLR as parents. Allowed by the datamodel; noted here for graph-walkers.
- **Repo files with no vault coverage:** `web/src/styles.css`, `web/index.html`, `web/vite.config.ts`, `web/tsconfig.json`, `web/package.json`. Build/styling concerns the vault deliberately omits — refer to repo files directly.
- **Backend ↔ frontend integration seams (explicit in the vault):**
  - `frontend/llr/API types mirror backend span-canonical model.md` → `backend/llr/API router exposes session and span endpoints.md` (and the wider Dashboard REST API HLR).
  - `frontend/llr/API client deleteSession uses DELETE.md` → `backend/llr/API delete session purges traces and projections.md`.
  - WS subscription LLRs trace upward to `backend/hlr/Live WebSocket Event Stream.md`.
  - Resolving frontend `api/` sources reaches backend HLRs `Dashboard REST API`, `Live WebSocket Event Stream`, `Span Normalization`, `Telemetry Persistence`, `Uniform Error Reporting`. The contract surface itself is in `web/src/api/types.ts`.
- **No `test/*`-tagged stubs** under `frontend/` (only under `backend/`). Frontend coverage is "untested in vault" across the board; do not infer CI coverage.
