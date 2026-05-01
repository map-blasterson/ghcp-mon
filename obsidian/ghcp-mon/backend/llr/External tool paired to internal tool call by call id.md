---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
When upserting a `tool_calls` row whose `call_id` is non-null, the normalizer MUST update any existing `external_tool_calls` rows with the same `call_id` whose `paired_tool_call_pk` is null, setting `paired_tool_call_pk` to the new tool_calls row's `tool_call_pk`. Symmetrically, when upserting an `external_tool_calls` row, it MUST look up the matching `tool_calls.tool_call_pk` by `call_id` and store it as `paired_tool_call_pk` if available.

## Rationale
Internal and external tool spans for the same logical invocation arrive in either order; pairing them lets the UI present a single tool call.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
