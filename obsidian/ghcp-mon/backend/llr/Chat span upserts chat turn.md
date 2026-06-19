---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For a span classified as `Chat`, the normalizer MUST upsert one row in `chat_turns` keyed by `span_pk`, populating `conversation_id`, `model` (preferring `gen_ai.request.model` over `gen_ai.response.model`), and the four token-usage counters: `input_tokens` from `gen_ai.usage.input_tokens`, `output_tokens` from `gen_ai.usage.output_tokens`, `cache_read_tokens` from `gen_ai.usage.cache_read_input_tokens` (else `gen_ai.usage.cache_read.input_tokens`), and `reasoning_tokens` from `gen_ai.usage.reasoning_output_tokens` (else `gen_ai.usage.reasoning.output_tokens`).

## Rationale
Chat turns drive the dashboard's per-turn token accounting and model-attribution displays.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
