---
type: impl
source: src/tui/scenarios/spans/invoke_agent.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Pure function `latest_chat_descendant(tree, agent_span_id)`: locates the agent node, walks its descendants for the latest chat span by `sort_key = (end_unix_ns, start_unix_ns, span_pk)`. Returns `(trace_id, span_id)` for the chat detail column to consume.

## Source For
- [[Spans invoke_agent selection routes to latest chat descendant]]
