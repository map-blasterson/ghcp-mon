---
type: HLR
tags:
  - req/hlr
  - domain/ws
---
The system broadcasts real-time normalization and ingestion events to all connected dashboard clients over a WebSocket so the UI can update without polling.

## Derived LLRs
- [[WS sends hello on connect]]
- [[WS forwards broadcast events to client]]
- [[WS responds to ping with pong]]
- [[WS closes on client close]]
- [[Broadcaster fan out via tokio broadcast channel]]
- [[Span normalize emits span and trace events]]
- [[Placeholder creation emits placeholder events]]
- [[Projection upserts emit derived events]]
- [[Session upsert emits derived session event]]
- [[Metric ingest emits raw metric event]]
