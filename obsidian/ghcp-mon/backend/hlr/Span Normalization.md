---
type: HLR
tags:
  - req/hlr
  - domain/normalize
---
The system treats spans as the canonical truth and idempotently reconciles them into normalized projections — agent runs, chat turns, tool calls, external tool calls, hook invocations, skill invocations, and context snapshots — establishing parent/child relationships and conversation membership inferred from the span ancestry.

## Derived LLRs
- [[Span upsert by trace and span id]]
- [[Span events idempotently replaced on span upsert]]
- [[Placeholder span for unseen parent]]
- [[Placeholder upgrade preserved across reingest]]
- [[Span name classified into kind class]]
- [[Invoke agent span upserts agent run]]
- [[Chat span upserts chat turn]]
- [[Execute tool span upserts tool call]]
- [[External tool span upserts external tool call]]
- [[External tool paired to internal tool call by call id]]
- [[Projection pointers resolved via ancestor walk]]
- [[Forward resolve descendants on parent arrival]]
- [[Effective conversation id inherited from ancestors]]
- [[Session upserted per conversation id]]
- [[Session counters refreshed on session upsert]]
- [[Hook start event derives hook invocation]]
- [[Hook end event completes hook invocation]]
- [[Skill invoked event records skill invocation]]
- [[Usage info event creates context snapshot]]
- [[Chat token usage attributes create context snapshot]]
- [[Chat turn tool count refreshed]]
- [[Metric data points persisted to metric_points]]
- [[Logs not normalized currently]]
