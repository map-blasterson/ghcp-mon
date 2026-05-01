# api — Memory Bank (frontend)

## Purpose
The `web/src/api/` area is the dashboard's single typed conduit to the backend. `client.ts` issues HTTP requests against a fixed base URL and returns shape-preserving TypeScript values mirroring the server's span-canonical model; `ws.ts` maintains a single shared WebSocket connection to the backend's live event stream and fans envelopes out to listeners; `types.ts` declares the wire types that both share with the rest of the frontend.

## Key behaviors

- Export `API_BASE = "http://127.0.0.1:4319"` and `WS_URL = "ws://127.0.0.1:4319/ws/events"`; every HTTP request issued by `api.*` is sent against `API_BASE`.

- The `getJson`/`deleteJson` helpers throw an `Error` with message `"<status> <statusText> for <path>"` whenever `fetch` returns a response with `ok === false`, and do not attempt to parse the response body in that case.

- The `qs` helper omits any entry whose value is `undefined`, `null`, or empty string; `encodeURIComponent`s both keys and values; joins entries with `&`; and returns the empty string (no leading `?`) when no entries remain.

- `api.listSessions` and `api.listTraces` default `limit` to `50` when the caller omits it; `api.listSpans` and `api.listRaw` default `limit` to `100`. The default is sent as the `limit` query parameter on the underlying request.

- `api.deleteSession(cid)` issues an HTTP `DELETE` to `/api/sessions/{cid}` and resolves to the parsed JSON body `{ deleted: boolean; conversation_id: string; trace_count: number }`.

- `web/src/api/types.ts` declares the TypeScript types the API client returns — `SessionSummary`, `SessionDetail`, `SpanRow`, `SpanFull`, `SpanDetail`, `SpanNode`, `SessionSpanTreeResponse`, `TraceSummary`, `TraceDetailResponse`, `ContextSnapshot`, `RawRecord`, the WS envelope/payload shapes, and the `KindClass` and `RawRecordType` string-literal unions — structurally matching the JSON emitted by the backend's `src/api/mod.rs` and `src/ws/*` modules.

- `wsBus` is exported as a singleton; `wsBus.start()` opens a `WebSocket` to `WS_URL` only on its first call. Subsequent calls while a socket already exists are no-ops.

- After a `close` event or constructor failure, `WsBus` schedules a reconnect after `min(30_000, 500 * 2^attempt)` ms, with `attempt` starting at 0 and incrementing on every scheduling. `attempt` is reset to 0 on a successful `open`.

- On every WebSocket `message` event, `WsBus` calls `JSON.parse` on the payload, treats the result as a `WsEnvelope`, and invokes every listener registered via `wsBus.on` with that envelope.

- If `JSON.parse` throws on an incoming `message`, `WsBus` swallows the error and does not invoke any listener for that message.

- `wsBus.onStatus(listener)` immediately invokes `listener` with the current `connected` boolean, then invokes `listener(true)` on every `open` and `listener(false)` on every `close`, and returns an unsubscribe function that removes the listener.

- On a `WebSocket` `error` event, `WsBus` calls `close()` on the underlying socket so the standard `close` handler fires the reconnect path.

## Public surface

From `client.ts`:
- `API_BASE: string` — `"http://127.0.0.1:4319"`.
- `WS_URL: string` — `"ws://127.0.0.1:4319/ws/events"`.
- `qs(params)` — query-string builder helper (skips empty values, encodes, returns `""` for empty).
- `getJson` / `deleteJson` — internal HTTP helpers that throw on non-2xx.
- `api.listSessions(...)` — list sessions, default `limit = 50`.
- `api.listTraces(...)` — list traces, default `limit = 50`.
- `api.listSpans(...)` — list spans, default `limit = 100`.
- `api.listRaw(...)` — list raw records, default `limit = 100`.
- `api.deleteSession(cid)` — DELETE `/api/sessions/{cid}`, returns `{ deleted, conversation_id, trace_count }`.
- Additional `api.*` getters exist for session/span/trace detail endpoints; the vault does not enumerate them — read source for the full method list.

From `ws.ts`:
- `wsBus` — singleton instance of `WsBus`.
  - `wsBus.start()` — idempotent connect.
  - `wsBus.on(listener)` — register an envelope listener; returns unsubscribe.
  - `wsBus.onStatus(listener)` — register a status listener invoked synchronously with the current state, then on each open/close; returns unsubscribe.
  - `wsBus.isConnected()` — current connection boolean (consumed by `useWsStatus`).

From `types.ts` (structural mirrors of backend wire shapes):
- `SessionSummary`, `SessionDetail`
- `SpanRow`, `SpanFull`, `SpanDetail`, `SpanNode`, `SessionSpanTreeResponse`
- `TraceSummary`, `TraceDetailResponse`
- `ContextSnapshot`
- `RawRecord`
- WS envelope/payload shapes (`WsEnvelope`, …)
- String-literal unions: `KindClass`, `RawRecordType`

## Invariants & constraints

- **Base URL is hardcoded.** No environment-driven indirection; the dashboard is bundled with and served by the backend on the same host/port.
- **Error semantics on HTTP.** Non-2xx → thrown `Error` with message `"<status> <statusText> for <path>"`; body is never read on error. This shape is what TanStack Query keys off to mark queries failed and to surface human-readable error toasts.
- **Query encoding.** Empty (`undefined` | `null` | `""`) entries dropped; both keys and values `encodeURIComponent`-encoded; `&`-joined; empty result returns `""` with no leading `?`. Avoids stray `param=` on the backend and prevents spurious cache-key churn in TanStack Query.
- **Default page sizes.** sessions/traces = 50; spans/raw = 100. Backend clamps independently.
- **Type drift detection.** Components are fully typed against `types.ts`; backend field changes surface as TypeScript build failures.
- **WS connection is a singleton.** Exactly one socket per dashboard tab; `start()` is idempotent; protects against hot-reload stampedes.
- **Reconnect policy.** Exponential backoff `500 * 2^attempt` ms, capped at 30 000 ms. Counter resets to 0 only on a successful `open`. Both `close` and constructor failure feed this path.
- **Failure funneling.** A `WebSocket` `error` event triggers `close()` so reconnect logic lives only in the `close` handler.
- **Malformed-frame tolerance.** `JSON.parse` failure is swallowed; listeners are not invoked for that message; the socket stays open.
- **Status delivery is synchronous-on-subscribe.** `onStatus` invokes the new listener with the current state immediately so consumers (status dot) avoid a flash of "disconnected".
- **No fan-out logic in `ws.ts`.** Per-`(kind, entity)` ring buffers, wildcard fan-out, and the React `useWsStatus`/`useLiveFeed` hooks are layered above this area (see Dependencies → Upstream).

## Dependencies

- **Upstream consumers:**
  - `web/src/state/live.ts` — registers a `wsBus.on` listener and maintains the per-`(kind, entity)` ring buffer (`RING_MAX = 500`, newest-first) that `useLiveFeed` reads from. Wakes both exact-key and `"*"` wildcard subscribers.
  - `web/src/hooks/useWsStatus` (or equivalent) — calls `wsBus.start()`, reads `wsBus.isConnected()`, and subscribes via `wsBus.onStatus` to re-render on status change.
  - React Query hooks throughout the components — call `api.*` and rely on its thrown-`Error`-on-non-2xx contract.
  - Components performing session deletion — call `api.deleteSession` and read `{ deleted, conversation_id, trace_count }`.

- **Downstream:**
  - Backend HTTP endpoints exposed by `src/api/mod.rs` (sessions, traces, spans, raw, session detail, span trees, etc.).
  - Backend WebSocket endpoint at `/ws/events` served by `src/ws/*`.
  - Browser `fetch` (HTTP) and `WebSocket` (live stream) APIs.

- **External libs (mentioned in requirements):**
  - **TanStack Query (React Query)** — referenced by the LLRs covering thrown errors and query-string stability; the API client's error and encoding semantics are tuned to its caching/error-handling behavior.

## Where to read for detail

- **Vault HLRs:**
  - `frontend/hlr/REST API Client.md`
  - `frontend/hlr/Live WebSocket Subscription.md`

- **Vault LLRs:**
  - `frontend/llr/API base URL hardcoded to local backend.md`
  - `frontend/llr/API client throws on non-2xx responses.md`
  - `frontend/llr/API client query string encoding.md`
  - `frontend/llr/API list methods apply default page size.md`
  - `frontend/llr/API client deleteSession uses DELETE.md`
  - `frontend/llr/API types mirror backend span-canonical model.md`
  - `frontend/llr/WS bus singleton lazy start.md`
  - `frontend/llr/WS reconnect with exponential backoff.md`
  - `frontend/llr/WS dispatches parsed envelopes to listeners.md`
  - `frontend/llr/WS ignores malformed JSON messages.md`
  - `frontend/llr/WS exposes connection status to subscribers.md`
  - `frontend/llr/WS error closes socket to trigger reconnect.md`
  - (Adjacent, layered above `api/` but linked from the WS HLR) `frontend/llr/Live feed ring buffer capped at 500 envelopes.md`, `frontend/llr/Live feed wakes filter and wildcard subscribers.md`, `frontend/llr/useWsStatus reflects current connection state.md`

- **Source files:** `web/src/api/client.ts`, `web/src/api/types.ts`, `web/src/api/ws.ts`
