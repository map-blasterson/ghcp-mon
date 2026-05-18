---
type: LLR
tags:
  - req/llr
  - domain/export
---
When `export::export_session` is called with a `conv_id` for which no row exists in the `sessions` table, it MUST return `Err(AppError::NotFound)` before writing any bytes to the writer.

## Rationale
A missing session must not produce a truncated or empty output file; the caller relies on this to abort before opening the destination.

## Derived from
- [[Session Export]]
