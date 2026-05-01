---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For a span classified as `InvokeAgent`, the normalizer MUST upsert one row in `agent_runs` keyed by `span_pk`, populating `agent_name` from the `gen_ai.agent.name` attribute (falling back to the suffix after `invoke_agent ` in the span name), `agent_id` from `gen_ai.agent.id`, `agent_version` from `gen_ai.agent.version`, and `conversation_id` from `gen_ai.conversation.id`, and coalescing pre-existing values on conflict.

## Rationale
Agent runs are the highest-level projection a client sees; values arriving on later re-deliveries must not erase earlier enrichment.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
