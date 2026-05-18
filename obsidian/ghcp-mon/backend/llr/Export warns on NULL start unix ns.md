---
type: LLR
tags:
  - req/llr
  - domain/export
---
When `export::export_session` encounters a selected span row with `start_unix_ns IS NULL`, it MUST emit a `tracing::warn!` event identifying the span's `span_pk` and `span_id`, and MUST still emit the envelope using `0` as the substituted start time so the row replays.

## Rationale
The schema permits NULL start times on real spans (defensive); the export should be resilient and visible — warn the operator but do not drop the row.

## Derived from
- [[Session Export]]
