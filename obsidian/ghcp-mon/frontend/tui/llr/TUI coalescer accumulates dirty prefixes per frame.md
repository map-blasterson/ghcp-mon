---
type: LLR
tags:
  - req/llr
  - tui
  - domain/event-loop
---
The WS coalescer task MUST, for each incoming `WsEnvelope`, look up the cache-key prefixes returned by `ws_invalidation_prefixes` and append each to a per-frame buffer (deduped). Once `FRAME_INTERVAL` elapses (and the buffer is non-empty), it MUST emit a single `AppEvent::WsTick { dirty_prefixes, envelopes }` carrying both.

## Rationale
Single packed update per frame for both live-feed ingestion and cache invalidation, instead of N events.

## Derived from
- [[Terminal Event Loop]]
