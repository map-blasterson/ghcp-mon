---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
`effective_conversation_id(span_pk)` MUST return the `gen_ai.conversation.id` value of the closest ancestor (including the span itself, depth 0) whose `attributes_json` carries that attribute, walking up to 64 levels, returning `None` when no ancestor in the chain carries it.

## Rationale
Tool and chat spans carry the conversation id directly; nested invoke_agent spans carry it on the parent and children must inherit.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
