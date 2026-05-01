---
type: LLR
tags:
  - req/llr
  - domain/otlp
---
The OTLP router MUST expose three POST routes — `/v1/traces`, `/v1/metrics`, and `/v1/logs` — handled by `ingest::otlp::traces`, `ingest::otlp::metrics`, and `ingest::otlp::logs` respectively.

## Rationale
Matches the OTLP/HTTP path convention so unmodified OTel exporters can target the receiver.

## Test context
- [[Server Router Cheatsheet]]

## Derived from
- [[OTLP HTTP Receiver]]

## Test case
- [[Server Router Tests]]
