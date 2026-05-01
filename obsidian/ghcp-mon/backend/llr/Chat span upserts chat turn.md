---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For a span classified as `Chat`, the normalizer MUST upsert one row in `chat_turns` keyed by `span_pk`, populating `conversation_id`, `interaction_id` (`github.copilot.interaction_id`), `turn_id` (`github.copilot.turn_id`), `model` (preferring `gen_ai.request.model` over `gen_ai.response.model`), and the four token-usage counters (`input_tokens`, `output_tokens`, `cache_read_tokens`, `reasoning_tokens`) from the corresponding `gen_ai.usage.*` attributes.

## Rationale
Chat turns drive the dashboard's per-turn token accounting and model-attribution displays.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
