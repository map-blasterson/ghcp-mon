---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-events
---
On any `tokio-tungstenite` stream error, the TUI `WsBus` MUST close the underlying socket so the standard reconnect path runs (mirrors the web `onerror -> close()` semantics).

## Rationale
Centralizes reconnect to one code path regardless of whether failure surfaces as error or close.

## Derived from
- [[Server URL Normalization and Reconnect Status]]
- [[WS error closes socket to trigger reconnect]]
