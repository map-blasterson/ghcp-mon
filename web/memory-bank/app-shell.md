# app-shell — Memory Bank (frontend)

## Purpose
The app shell is the React entry point and global-state bootstrap for the
dashboard. It mounts the `<App />` tree under `StrictMode` +
`QueryClientProvider`, owns the singleton `QueryClient` (with TanStack Query
defaults tuned for a WS-driven app), and renders the top-bar control surface
that sits above `Workspace` — scenario-type "add column" selector and live
WS connection indicator. It is the smallest area in the frontend: two source
files, four LLRs, all derived from a single HLR (`Workspace Layout`).

## Key behaviors

- **React entry & providers (`main.tsx`).** Mount `<App />` under
  `React.StrictMode` wrapped in a `QueryClientProvider`, into the DOM
  element with id `root`. StrictMode catches dev-time anti-patterns;
  `QueryClientProvider` is the root for every TanStack Query in the app.
  — `frontend/llr/App bootstraps React StrictMode and TanStack QueryClient.md`

- **Shared `QueryClient` defaults (`main.tsx`).** The `QueryClient`
  constructed in `main.tsx` sets `defaultOptions.queries` to
  `{ refetchOnWindowFocus: false, retry: 1, staleTime: 5_000 }`. Rationale:
  live invalidation is driven by the WS feed, so focus refetch is
  unnecessary; the 5 s stale window dampens redundant fetches across
  columns that share a query key.
  — `frontend/llr/Default TanStack Query options no refetch on focus.md`

- **Top bar — add column selector (`App.tsx`).** Render a `<select>` whose
  options are `Object.entries(SCENARIO_LABELS)`. Selecting an option
  appends a new `Column` to the workspace with that scenario type, an id
  from `genId()`, the matching label as title, an empty `config`, and
  `width: 1.2`, then resets the select back to the empty placeholder. This
  is the primary user-facing workspace mutation; the placeholder reset
  returns the control to its idle state.
  — `frontend/llr/Top bar adds columns selects scenario type.md`

- **Top bar — WS status dot (`App.tsx`).** Render a status dot whose `on`
  class is present iff `useWsStatus()` returns `true`, with `title`
  `"connected"` when on and `"disconnected"` when off. Always-visible
  indicator of live-feed health.
  — `frontend/llr/Top bar exposes ws connection indicator.md`

## Public surface

- **`App` component** (`web/src/App.tsx`) — top-bar shell + `Workspace`
  host. Exposes the "add column" `<select>` driven by `SCENARIO_LABELS`
  and the WS connection indicator driven by `useWsStatus()`.
- **`main.tsx` entry** (`web/src/main.tsx`) — DOM mount point, StrictMode
  wrapper, `QueryClient` construction, `QueryClientProvider`.
- No exports from this area are consumed by other frontend modules; the
  shell is a leaf at the top of the tree.

## Invariants & constraints

- Mount target is the DOM element with id `root`. (LLR: *App bootstraps
  React StrictMode and TanStack QueryClient*)
- Exactly one `QueryClient`, constructed in `main.tsx` and shared via
  `QueryClientProvider`. (Same LLR.)
- `defaultOptions.queries.refetchOnWindowFocus` MUST be `false`;
  `retry` MUST be `1`; `staleTime` MUST be `5_000` ms. (LLR: *Default
  TanStack Query options no refetch on focus*)
- New columns from the top-bar selector MUST be created with: scenario
  type from the chosen option, `id = genId()`, `title` = matching label
  from `SCENARIO_LABELS`, `config = {}`, `width = 1.2`. After append, the
  `<select>` value MUST reset to the empty placeholder. (LLR: *Top bar
  adds columns selects scenario type*)
- The status dot's `on` class is present iff `useWsStatus() === true`;
  `title` is `"connected"` when on, `"disconnected"` when off — no other
  states. (LLR: *Top bar exposes ws connection indicator*)
- StrictMode wraps the entire app — components in this tree must be
  StrictMode-safe (idempotent effects, no setState-in-render).

## Dependencies

- **Mounts** `components/Workspace` (the column layout / persistence /
  resizing surface — covered by the other 11 LLRs under
  `frontend/hlr/Workspace Layout.md`).
- **Reads** `useWsStatus()` for the connection indicator — the WS hook
  lives outside this area.
- **Reads** `SCENARIO_LABELS` (and the implied scenario registry) to
  populate the add-column selector. The registry itself is governed by
  `frontend/llr/Scenario registry maps scenario type to component.md`,
  not by this area.
- **Uses** `genId()` for new column ids and the workspace mutation API
  (append-column) exposed by the Workspace state owner.
- **Provides** the TanStack `QueryClient` consumed by every query hook
  in the app.

## Repo files outside vault scope

The following files are present in the repo but have **no impl note and
no requirements** in the vault. They are build / styling / tooling
concerns; treat the files themselves as the source of truth and do not
expect normative statements in the requirements graph.

- `web/src/styles.css` — global CSS for the app shell and workspace.
- `web/index.html` — Vite HTML entry; hosts the `#root` mount node that
  `main.tsx` targets.
- `web/vite.config.ts` — Vite dev/build configuration (dev server, proxy,
  bundling).
- `web/tsconfig.json` — TypeScript compiler configuration for the web
  package.
- `web/package.json` — npm dependencies and scripts.

If any of these grow behavior worth pinning (e.g. a dev-proxy contract,
a CSS class invariant the components depend on), promote it to an LLR
under `frontend/hlr/Workspace Layout.md` (or a new HLR) before relying
on it from elsewhere.

## Where to read for detail

**HLR**
- `frontend/hlr/Workspace Layout.md` — parent of all four LLRs in this
  area (and 10 more covering the rest of the workspace).

**LLRs (this area)**
- `frontend/llr/App bootstraps React StrictMode and TanStack QueryClient.md`
- `frontend/llr/Default TanStack Query options no refetch on focus.md`
- `frontend/llr/Top bar adds columns selects scenario type.md`
- `frontend/llr/Top bar exposes ws connection indicator.md`

**Source**
- `web/src/main.tsx` — entry, StrictMode, `QueryClient`, provider mount.
- `web/src/App.tsx` — top-bar shell, add-column selector, WS status dot,
  hosts `Workspace`.
