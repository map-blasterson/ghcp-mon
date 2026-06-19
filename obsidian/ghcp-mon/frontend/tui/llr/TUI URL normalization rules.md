---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-events
---
`normalize_server(server)` MUST strip trailing `/` from the input, MUST reject any input whose scheme is not `http` or `https`, MUST return the REST base unchanged in scheme, and MUST derive the WS URL by substituting `http→ws` / `https→wss` and appending `/ws/events`.

## Rationale
Single source of truth for both the REST client base and the WS bus URL.

## Derived from
- [[Server URL Normalization and Reconnect Status]]
