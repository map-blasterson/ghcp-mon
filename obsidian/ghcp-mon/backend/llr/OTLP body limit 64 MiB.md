---
type: LLR
tags:
  - req/llr
  - domain/otlp
---
The OTLP router MUST permit request bodies up to 64 MiB (`64 * 1024 * 1024` bytes) via the axum `DefaultBodyLimit` layer.

## Rationale
Default axum limit (2 MiB) is too small for batched OTLP traces from real CLI sessions.

## Test context
- [[Server Router Cheatsheet]]

## Derived from
- [[OTLP HTTP Receiver]]

## Test case
- [[Server Router Tests]]
