---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For a span classified as `ExternalTool`, the normalizer MUST upsert one row in `external_tool_calls` keyed by `span_pk`, taking `call_id` from `github.copilot.external_tool.call_id` (falling back to `gen_ai.tool.call.id`) and `tool_name` from `github.copilot.external_tool.name` (falling back to `gen_ai.tool.name`).

## Rationale
External tool spans use a copilot-specific attribute namespace that aliases the generic gen_ai keys.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
