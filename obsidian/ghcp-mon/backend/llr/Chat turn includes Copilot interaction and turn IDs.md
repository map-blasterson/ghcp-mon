---
type: LLR
tags:
  - req/llr
  - domain/normalize
  - github-specific
---
When upserting a `chat_turns` row, the normalizer MUST also populate `interaction_id` from `github.copilot.interaction_id` and `turn_id` from `github.copilot.turn_id`.

## Rationale
Copilot interaction and turn identifiers let the dashboard correlate chat turns with the Copilot session model's own numbering.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
