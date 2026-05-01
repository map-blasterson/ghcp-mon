---
type: LLR
tags:
  - req/llr
  - domain/db
  - domain/otlp
---
For every OTLP/HTTP request accepted by the receiver, the handler MUST persist the raw request body as exactly one `raw_records` row with `source='otlp-http-json'` and `content_type='application/json'` before deriving any envelopes.

## Rationale
Audit + replay-from-archive: the wire request must be reconstructible regardless of how many envelopes it produced.

## Test context
- [[Ingest Pipeline Cheatsheet]]
- [[OTLP Handlers Cheatsheet]]

## Derived from
- [[Telemetry Persistence]]
- [[OTLP HTTP Receiver]]

## Test case
- [[Ingest Pipeline Tests]]
