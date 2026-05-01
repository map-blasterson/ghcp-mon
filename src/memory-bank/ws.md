# ws — Memory Bank (backend)

## Purpose
The `ws` area provides a `tokio::sync::broadcast`-backed fan-out (`Broadcaster`) plus a per-client Axum WebSocket handler mounted at `/ws/events`. Together they push real-time normalization and ingestion events to every connected dashboard client without polling, with a fixed lifecycle: hello → forward → ping/pong → close.

## Key behaviors
- `Broadcaster::new(cap)` wraps a `tokio::sync::broadcast` channel of the supplied capacity; `Broadcaster::send(msg)` fans out to every current subscriber and silently ignores the no-subscriber error so producers never block when no UI is connected — `Broadcaster fan out via tokio broadcast channel`.
- On accepting an upgrade at `/ws/events`, the handler immediately sends the JSON text frame `{"kind":"hello","entity":"control","payload":{"server":"ghcp-mon"}}` before any forwarded event; if that send fails the connection is closed — `WS sends hello on connect`.
- After the hello, every `EventMsg` received from the broadcaster subscription is forwarded to the client as a JSON text frame; any send failure exits the per-client loop and closes the socket so a slow/broken client never stalls the bus or leaks the task — `WS forwards broadcast events to client`.
- An incoming WebSocket `Ping` frame is answered with a `Pong` frame echoing the same payload bytes (standard keepalive expected by browsers and intermediaries) — `WS responds to ping with pong`.
- The handler exits its loop cleanly when the client sends a `Close` frame or the receive stream yields `None` — `WS closes on client close`.

## Public surface
- `Broadcaster::new(cap)` — wraps `tokio::sync::broadcast` with the given capacity.
- `Broadcaster::send(EventMsg)` — fan-out; ignores the no-subscriber error.
- `Broadcaster::subscribe()` — used by the per-client WS handler to obtain a receiver.
- `EventMsg` shape: `{ kind, entity, payload }` (serialized as a JSON text frame on the wire).
- WebSocket endpoint: `/ws/events` (route mounted in `server.rs`).

## Invariants & constraints
- **Hello-first:** the hello frame `{"kind":"hello","entity":"control","payload":{"server":"ghcp-mon"}}` MUST precede any forwarded event on a new connection; failure to send it closes the connection.
- **JSON text frames:** every `EventMsg` is forwarded as a JSON text frame; a send error terminates the per-client loop and closes the socket.
- **Keepalive:** `Ping(payload)` → `Pong(payload)` with the same payload bytes.
- **Clean teardown:** `Close` frame or end-of-stream (`None`) ends the per-client loop without affecting other clients or the broadcaster.
- **Non-blocking producers:** `Broadcaster::send` ignores the no-subscriber error so producers never block on the absence of clients.
- **Per-client isolation:** a failing/slow client only tears down its own task; the shared broadcast channel and other subscribers are unaffected.

## Dependencies
- **Producers (upstream) — emit `EventMsg` into `Broadcaster`:**
  - `normalize/` — emits envelopes for the kinds:
    - `span` and `trace` (`Span normalize emits span and trace events`)
    - `placeholder` (`Placeholder creation emits placeholder events`)
    - `derived` per-projection (`Projection upserts emit derived events`)
    - `derived/session` (`Session upsert emits derived session event`)
    - `metric` (`Metric ingest emits raw metric event`)
  - `api/` — `API delete session purges traces and projections` (delete-session emit).
- **Routing:** `server.rs` mounts the handler at `/ws/events` and injects the shared `Broadcaster` into handler state.
- **Consumed by:** the frontend `wsBus` singleton and `state/live.ts` ring-buffered fan-out (see `web/memory-bank/api.md` and `web/memory-bank/state.md`).

## Where to read for detail
- HLR: `backend/hlr/Live WebSocket Event Stream.md`
- LLRs (5):
  - `backend/llr/Broadcaster fan out via tokio broadcast channel.md`
  - `backend/llr/WS sends hello on connect.md`
  - `backend/llr/WS forwards broadcast events to client.md`
  - `backend/llr/WS responds to ping with pong.md`
  - `backend/llr/WS closes on client close.md`
- Source: `src/ws/mod.rs` (`Broadcaster`, `EventMsg`), `src/ws/handler.rs` (per-client lifecycle).
- Cross-references — producers:
  - `backend/llr/Span normalize emits span and trace events.md`
  - `backend/llr/Placeholder creation emits placeholder events.md`
  - `backend/llr/Projection upserts emit derived events.md`
  - `backend/llr/Session upsert emits derived session event.md`
  - `backend/llr/Metric ingest emits raw metric event.md`
  - `backend/llr/API delete session purges traces and projections.md`
- Route mount: `src/server.rs` (`/ws/events`).
