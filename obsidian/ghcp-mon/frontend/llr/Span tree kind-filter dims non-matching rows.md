---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When `kind_filter` is set in session-tree mode and no search is active, `SpanTreeNode` MUST apply the `kind-dim` CSS class (opacity 0.4) to every row whose `kind_class` does not equal the filter value, but MUST NOT hide those rows.

## Rationale
Dimming (rather than hiding) preserves tree structure context and prevents disorientation from partially-ingested spans that may not yet have a final kind assignment.

## Derived from
- [[Trace and Span Explorer]]
