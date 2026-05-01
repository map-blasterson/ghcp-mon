---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
When a span resolves to a non-null effective conversation id, the normalizer MUST upsert a `sessions` row keyed by `conversation_id`, with `first_seen_ns = MIN(existing, new)`, `last_seen_ns = MAX(existing, new)`, and `latest_model` coalescing to the just-observed model when present.

## Rationale
Sessions are the user-visible aggregation of all spans sharing a conversation id.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
