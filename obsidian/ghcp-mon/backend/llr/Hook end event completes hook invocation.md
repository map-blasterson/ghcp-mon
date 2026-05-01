---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For each span event named `github.copilot.hook.end`, the normalizer MUST upsert the matching `hook_invocations` row by `invocation_id`, setting `end_unix_ns` and computing `duration_ns = end_unix_ns - start_unix_ns` only when `start_unix_ns` is already set.

## Rationale
Duration is meaningful only after both start and end events have been observed.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
