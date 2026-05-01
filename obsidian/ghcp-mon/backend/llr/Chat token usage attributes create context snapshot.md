---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For a `Chat` span carrying any of `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.usage.cache_read.input_tokens`, or `gen_ai.usage.reasoning.output_tokens`, the normalizer MUST upsert a `context_snapshots` row with `source='chat_span'`, `captured_ns = end_unix_ns ?? start_unix_ns`, and the four token counters, coalescing each on conflict against `(span_pk, source, captured_ns)`.

## Rationale
Per-turn token usage is a derived snapshot that supplements the raw chat_turns row for time-series displays.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
