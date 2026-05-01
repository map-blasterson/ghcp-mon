---
type: impl
source: src/normalize/mod.rs
lang: rust
tags:
  - impl/original
  - impl/rust
---
Original source file for reverse-engineered requirements.

## Source For
- [[Span upsert by trace and span id]]
- [[Span events idempotently replaced on span upsert]]
- [[Placeholder span for unseen parent]]
- [[Placeholder upgrade preserved across reingest]]
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
- [[Span normalize emits span and trace events]]
- [[Placeholder creation emits placeholder events]]
- [[Projection upserts emit derived events]]
- [[Session upsert emits derived session event]]
- [[Metric ingest emits raw metric event]]
