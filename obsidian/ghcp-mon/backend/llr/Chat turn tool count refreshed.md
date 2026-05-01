---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
After updating projection tables for a span, the normalizer MUST refresh `tool_call_count` on every `chat_turns` row whose `turn_pk` is reachable from this span — directly via `chat_turns.span_pk = span_pk`, or indirectly via `tool_calls.span_pk = span_pk`/`external_tool_calls.span_pk = span_pk` — by counting `tool_calls` rows whose `chat_turn_pk` matches.

## Rationale
A new tool span anywhere under a chat turn must update that turn's denormalized counter.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
