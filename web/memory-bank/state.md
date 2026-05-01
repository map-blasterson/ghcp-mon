# state — Memory Bank (frontend)

## Purpose
The `web/src/state/` module is a small but architecturally pivotal slice: 7 LLRs across 3 files, where **each file is the single concrete implementation of a different HLR**. That 1:1 file↔HLR mapping is the whole point of the module — it is the seam between the three top-level dashboard concerns (workspace shell, live event fan-out, cross-column hover) and the React component tree. Everything else in the frontend reads from these stores; almost nothing writes to them outside this folder.

File ↔ HLR ↔ LLR-count map:

| File | HLR | LLRs in scope |
|---|---|---|
| `workspace.ts` | `frontend/hlr/Workspace Layout.md` | 3 |
| `live.ts` | `frontend/hlr/Live WebSocket Subscription.md` | 3 |
| `hover.ts` | `frontend/hlr/Context Growth Widget.md` | 1 |

## Key behaviors

### File: workspace.ts
Backs the persistent dashboard shell — column list, ordering/widths/titles, plus the bottom Context Growth widget's height and visibility. Implemented as a Zustand store wrapped with the `persist` middleware.

- **Persisted shape via Zustand `persist` middleware.** The `useWorkspace` store MUST be wrapped with `persist` under storage key `"ghcp-mon-workspace-v6"`, persisting exactly three fields across reloads: `columns`, `contextWidgetHeightVh`, and `contextWidgetVisible`. The `-v6` key suffix is intentional cache-busting: bumping the suffix invalidates incompatible older snapshots in users' browsers without runtime reconciliation.
  — `frontend/llr/Workspace persists columns to localStorage.md`

- **Migration drops obsolete scenario types.** The `persist` `migrate` callback MUST filter out any persisted column whose `scenarioType` is in the obsolete set `{"context_growth", "tool_registry", "context_inspector", "shell_io"}` before returning the rehydrated state. These four scenario types existed in earlier app versions; if they leaked through, the `ColumnBody` switch would have no case and crash the column. Migration is therefore a *deletion* pass, not a transform.
  — `frontend/llr/Workspace migration drops obsolete scenario types.md`

- **Default layout = exactly four columns.** On first run (no persisted state) and on `resetDefault()`, the workspace MUST contain four columns in this exact order:
  1. `live_sessions` — width `1`, title `"Sessions"`
  2. `spans` — width `1.4`, title `"Traces"`
  3. `tool_detail` — width `1.4`, title `"Tool detail"`
  4. `chat_detail` — width `1.6`, title `"Chat detail"`
  Additionally `resetDefault()` MUST set `contextWidgetHeightVh = 15` and `contextWidgetVisible = true`. This is the canonical out-of-the-box dashboard.
  — `frontend/llr/Default workspace seeds four columns.md`

### File: live.ts
The dashboard's live-data fan-out layer. Wraps the singleton `wsBus` (from `web/src/api/ws.ts`) with per-`(kind, entity)` ring buffers and a React-friendly subscription hook. This is the *only* place components should obtain real-time event streams.

- **Per-key ring buffer, newest-first, capped at 500.** For each distinct `kind/entity` key, the module MUST maintain a ring of the most recent envelopes ordered newest-first, and MUST cap that ring at `RING_MAX = 500` envelopes — when a new envelope would push the ring past the cap, the oldest entries are truncated from the tail. This bounds memory in long-running dashboard sessions (the dashboard is expected to run for hours) while preserving the newest-first ordering that the list scenarios already render.
  — `frontend/llr/Live feed ring buffer capped at 500 envelopes.md`

- **Wildcard + filter subscriber wakeup.** When an envelope with key `kind/entity` arrives, `useLiveFeed` MUST notify:
  1. every subscriber registered under that **exact** `kind/entity` key, and
  2. every subscriber registered under the wildcard key `"*"`,
  by invoking each registered callback exactly once. Components use targeted keys for narrow invalidation (e.g. one detail panel watches one entity); the wildcard channel is reserved for whole-workspace listeners that need to react to any event.
  — `frontend/llr/Live feed wakes filter and wildcard subscribers.md`

- **`useWsStatus()` hook reflects bus connection state.** MUST return a boolean equal to `wsBus.isConnected()` on first render, MUST start the bus if it is not already running (lazy-start on first observer), and MUST cause the consuming component to re-render whenever the bus's connection status flips. The top-bar connection indicator dot consumes this hook directly — there is no prop drilling of WS state.
  — `frontend/llr/useWsStatus reflects current connection state.md`

### File: hover.ts
The smallest of the three: a single transient cross-column hover channel. Exists so the Spans column and the Context Growth widget at the bottom of the workspace can highlight a matching chat without coupling.

- **`useHoverState` Zustand store, deliberately non-persisted.** MUST expose a `hoveredChatPk: number | null` field together with a `setHoveredChatPk(pk)` setter, implemented as a Zustand store. The value MUST NOT be persisted across reloads — persistence would carry a stale highlight into the next session, where the referenced chat may no longer exist or may belong to a different scenario layout. Hover is by definition ephemeral.
  — `frontend/llr/Hover store publishes hovered chat pk.md`

## Public surface

### workspace.ts
- `useWorkspace` — Zustand hook; persisted store with `columns`, `contextWidgetHeightVh`, `contextWidgetVisible` plus mutators (e.g. `resetDefault`, column add/rename/move/remove, weight rebalancers — used by Top bar, Workspace, Resizer, Column header). LocalStorage key: `"ghcp-mon-workspace-v6"`.
- Column / scenario type shapes used by the persisted state (consumed by `ColumnBody` and the scenario registry).

### live.ts
- `useLiveFeed(kind, entity)` — React hook returning the per-key ring of envelopes (newest first, cap 500), with subscriber wakeup on matching or wildcard `"*"` events.
- `useWsStatus()` — React hook returning `boolean` connection status; lazy-starts the bus on first call.
- (Subscriber registration is internal to the hook; components do not register callbacks directly.)

### hover.ts
- `useHoverState` — Zustand hook exposing `{ hoveredChatPk: number | null, setHoveredChatPk: (pk: number | null) => void }`. Not persisted.

## Invariants & constraints

- **localStorage round-trip + obsolete-scenario migration.** Persistence is keyed on `"ghcp-mon-workspace-v6"` and migration on rehydrate strips obsolete `scenarioType` values (`context_growth`, `tool_registry`, `context_inspector`, `shell_io`) before they reach `ColumnBody`. Bumping the version suffix is the supported way to invalidate snapshots wholesale; the migration callback is the supported way to surgically drop entries without a version bump.
- **Default 4-column seed is canonical.** First-run AND `resetDefault()` produce *exactly* four columns in the fixed order `live_sessions / spans / tool_detail / chat_detail` with widths `1 / 1.4 / 1.4 / 1.6` and titles `Sessions / Traces / Tool detail / Chat detail`. `resetDefault()` additionally forces `contextWidgetHeightVh = 15` and `contextWidgetVisible = true`. Tests and any "factory reset" path depend on this exact tuple.
- **Ring buffer cap = 500 per `(kind, entity)`.** Hard upper bound; tail-truncated on overflow; ordering newest-first is part of the contract (consumers do not re-sort).
- **Wildcard + filter subscriber wakeup semantics.** An envelope `kind/entity` wakes (a) exact-key subscribers and (b) `"*"` subscribers — each callback is called exactly once. There is no broader pattern matching; only exact key or `"*"`.
- **Hover store cross-column publish, never persisted.** `hoveredChatPk` is intentionally session-scoped. Do not add it to any persisted store.
- **`useWsStatus` lazy-starts the bus.** Mounting any component that calls `useWsStatus()` is sufficient to bring the WebSocket connection up; the module relies on this implicit start path rather than an explicit boot step in `App`.

## Dependencies

- **`live.ts` wraps `web/src/api/ws.ts`.** It is the only consumer-facing surface for the WebSocket bus; components should not import `wsBus` directly. The bus's connection lifecycle (lazy start, exponential-backoff reconnect, malformed-JSON guard, error-closes-to-trigger-reconnect) is owned by `api/ws.ts` and exposed here only as `useWsStatus()` and ring/subscription mechanics on top of envelope dispatch.
- **`workspace.ts` is consumed by:** the top bar (add column, reset, status dot adjacency), the workspace layout (column rendering, ResizeObserver-driven weighting), the resizer (rebalances adjacent column weights), the column header (rename / move / remove), and `ColumnBody` (which dispatches by `scenarioType` against the scenario registry). Every workspace-shell LLR under `Workspace Layout` ultimately reads or writes this store.
- **`hover.ts` is consumed by:** the Spans column (publishes hovered chat) and the Context Growth widget (highlights matching column on hover). It is the cross-column channel for the Context Growth Widget HLR.
- **No state file depends on another.** `workspace.ts`, `live.ts`, and `hover.ts` are independent — they share no imports, no derived state, no events. Their only common dependency is React + Zustand.

## Where to read for detail

### HLRs
- `frontend/hlr/Workspace Layout.md` — full dashboard shell HLR (workspace.ts is one of several files implementing it; this module owns the persistence + seeding LLRs).
- `frontend/hlr/Live WebSocket Subscription.md` — full live-events HLR (live.ts owns the ring-buffer + subscriber-wakeup + status-hook LLRs; the bus lifecycle LLRs live under `api/ws.ts`).
- `frontend/hlr/Context Growth Widget.md` — full widget HLR (hover.ts owns only the cross-column hover-publish LLR; the chart, resize, visibility, and invalidation LLRs live in widget components).

### LLRs implemented in this module
**workspace.ts**
- `frontend/llr/Workspace persists columns to localStorage.md`
- `frontend/llr/Workspace migration drops obsolete scenario types.md`
- `frontend/llr/Default workspace seeds four columns.md`

**live.ts**
- `frontend/llr/Live feed ring buffer capped at 500 envelopes.md`
- `frontend/llr/Live feed wakes filter and wildcard subscribers.md`
- `frontend/llr/useWsStatus reflects current connection state.md`

**hover.ts**
- `frontend/llr/Hover store publishes hovered chat pk.md`

### Impl notes (source ↔ requirement bridge)
- `frontend/impl/state__workspace.ts.md` → `web/src/state/workspace.ts`
- `frontend/impl/state__live.ts.md` → `web/src/state/live.ts`
- `frontend/impl/state__hover.ts.md` → `web/src/state/hover.ts`

### Source files
- `web/src/state/workspace.ts`
- `web/src/state/live.ts`
- `web/src/state/hover.ts`