---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For each span event named `github.copilot.session.usage_info`, the normalizer MUST upsert a `context_snapshots` row with `source='usage_info_event'` keyed by `(span_pk, source, captured_ns)`, capturing `token_limit`, `current_tokens`, and `messages_length` from the event attributes and coalescing each on conflict.

## Rationale
Context-window pressure is reported by the CLI as discrete events; deduplicating by capture timestamp keeps replay idempotent.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
