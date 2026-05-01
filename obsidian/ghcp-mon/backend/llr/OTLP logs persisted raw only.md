---
type: LLR
tags:
  - req/llr
  - domain/otlp
---
On a POST to `/v1/logs`, the handler MUST persist the raw request body with `record_type='otlp-logs'` and respond with `{"partialSuccess": {}, "accepted": 0}` without producing any normalized rows.

## Rationale
Logs are archived but not yet normalized; explicit `accepted: 0` signals to clients that no derived rows were created.

## Test context
- [[OTLP Handlers Cheatsheet]]

## Derived from
- [[OTLP HTTP Receiver]]

## Test case
- [[OTLP Handlers Tests]]
