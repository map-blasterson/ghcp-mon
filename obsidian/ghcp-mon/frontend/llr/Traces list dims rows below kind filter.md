---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When `kind_filter` is set in trace-list mode, the `TracesList` MUST render rows whose `kind_counts[kind_filter]` is `0` with the `dim` CSS class but MUST NOT hide them.

## Rationale
Hiding rows would mask placeholder/partially-ingested traces; dimming preserves situational awareness.

## Derived from
- [[Trace and Span Explorer]]
