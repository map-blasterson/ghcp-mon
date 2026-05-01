---
type: LLR
tags:
  - req/llr
  - domain/otlp
---
When unwrapping an OTLP `AnyValue` whose `intValue` is encoded as a string (per OTLP/JSON spec), the flatten routine MUST parse it as `i64` and emit a JSON number; if parsing fails it MAY pass the original value through unchanged.

## Rationale
OTLP/JSON encodes int64 as decimal string; downstream queries expect numeric values.

## Test context
- [[Ingest Pipeline Cheatsheet]]

## Derived from
- [[OTLP HTTP Receiver]]

## Test case
- [[Ingest Pipeline Tests]]
