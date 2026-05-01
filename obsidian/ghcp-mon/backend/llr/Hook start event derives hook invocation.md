---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For each span event named `github.copilot.hook.start`, the normalizer MUST upsert a `hook_invocations` row keyed by `invocation_id` (from `github.copilot.hook.invocation_id`), recording `hook_type`, `span_pk`, `conversation_id`, and `start_unix_ns` from the event's time, coalescing `conversation_id` on conflict.

## Rationale
Hook invocations span pre/post events whose start may arrive separately from the end.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
