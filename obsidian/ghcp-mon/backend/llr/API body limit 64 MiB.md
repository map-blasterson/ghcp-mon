---
type: LLR
tags:
  - req/llr
  - domain/api
---
The API router MUST permit request bodies up to 64 MiB (`64 * 1024 * 1024` bytes) via the axum `DefaultBodyLimit` layer.

## Rationale
The `/api/replay` POST endpoint accepts file-exporter JSON-lines payloads that can be large. The default axum body limit (2 MiB) is too small for realistic replay files.

## Derived from
- [[Dashboard REST API]]
