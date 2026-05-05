---
type: LLR
tags:
  - req/llr
  - domain/traces
---
`SpansScenario` MUST build a `nodeMap` (`Map<string, SpanNode>`) by walking the full session tree once per tree change, providing O(1) lookup of any span by `span_id` for follow-mode, search, and agent→chat routing logic.

## Rationale
Avoids repeated O(n) tree traversals when multiple features (follow-mode auto-advance, invoke_agent routing, search hit expansion) need to locate a span by ID within the same render cycle.

## Derived from
- [[Trace and Span Explorer]]
