---
type: LLR
tags:
  - req/llr
  - domain/otlp
---
On a successful POST to `/v1/metrics`, the handler MUST persist the raw request body once with `record_type='otlp-metrics'`, convert each metric into a `MetricEnvelope` (one envelope per metric, with a data-point list per envelope), ingest each via `ingest_envelope`, and respond with `{"partialSuccess": {}, "accepted": <n>}`.

## Rationale
Mirror of the traces path so dashboards can correlate metrics with the same raw-record audit trail.

## Test context
- [[OTLP Handlers Cheatsheet]]

## Derived from
- [[OTLP HTTP Receiver]]
- [[Telemetry Persistence]]

## Test case
- [[OTLP Handlers Tests]]
