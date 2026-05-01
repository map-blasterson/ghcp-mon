---
type: LLR
tags:
  - req/llr
  - domain/ws
  - domain/normalize
---
On every `sessions` upsert, the normalizer MUST broadcast a `kind="derived"`, `entity="session"` event with `action="update"`, `conversation_id`, and `latest_model`.

The legal `action` values for `derived`/`session` events are `update` (this requirement) and `delete` (see [[API delete session purges traces and projections]]); producers MUST NOT emit other values.

## Rationale
Drives live session-list updates.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
