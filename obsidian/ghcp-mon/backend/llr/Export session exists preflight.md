---
type: LLR
tags:
  - req/llr
  - domain/export
---
`export::session_exists(pool, conv_id)` MUST return `Ok(true)` when a row exists in the `sessions` table with `conversation_id = conv_id`, and `Ok(false)` otherwise, performing a single `SELECT 1 FROM sessions WHERE conversation_id = ? LIMIT 1` query.

## Rationale
Callers use this preflight to avoid creating or truncating an output file when the requested session does not exist.

## Derived from
- [[Session Export]]
