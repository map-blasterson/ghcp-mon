---
type: LLR
tags:
  - req/llr
  - domain/otlp
---
On a successful POST to `/v1/traces`, the handler MUST persist the raw request body once with `record_type='otlp-traces'` and `source='otlp-http-json'`, convert each span into a `SpanEnvelope`, ingest each envelope via `ingest_envelope`, and respond with `{"partialSuccess": {}, "accepted": <n>}` where `<n>` is the count of envelopes ingested.

## Rationale
Preserves the original wire bytes for audit while producing per-envelope rows for normalization.

## Test context
- [[OTLP Handlers Cheatsheet]]

## Derived from
- [[OTLP HTTP Receiver]]
- [[Telemetry Persistence]]

## Test case
- [[OTLP Handlers Tests]]
