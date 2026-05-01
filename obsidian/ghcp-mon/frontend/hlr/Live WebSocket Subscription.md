---
type: HLR
tags:
  - req/hlr
  - domain/live-events
---
The dashboard maintains a single WebSocket connection to the backend's live event stream, fans envelopes out to per-`(kind, entity)` ring buffers, and re-renders subscribed views on every matching envelope so the UI stays in sync with ingestion in real time.

## Derived LLRs
- [[WS bus singleton lazy start]]
- [[WS reconnect with exponential backoff]]
- [[WS dispatches parsed envelopes to listeners]]
- [[WS ignores malformed JSON messages]]
- [[WS exposes connection status to subscribers]]
- [[WS error closes socket to trigger reconnect]]
- [[Live feed ring buffer capped at 500 envelopes]]
- [[Live feed wakes filter and wildcard subscribers]]
- [[useWsStatus reflects current connection state]]
