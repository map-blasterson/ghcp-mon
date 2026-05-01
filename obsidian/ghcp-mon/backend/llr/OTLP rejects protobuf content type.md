---
type: LLR
tags:
  - req/llr
  - domain/otlp
---
When an OTLP request `Content-Type` header (case-insensitively) contains `application/x-protobuf`, the receiver MUST respond with HTTP 501 Not Implemented and a JSON error body explaining that protobuf ingestion is not implemented.

## Rationale
Only JSON OTLP is supported; protobuf clients should fail loudly rather than silently mis-parsing.

## Test context
- [[OTLP Handlers Cheatsheet]]

## Derived from
- [[OTLP HTTP Receiver]]
- [[Uniform Error Reporting]]

## Test case
- [[OTLP Handlers Tests]]
