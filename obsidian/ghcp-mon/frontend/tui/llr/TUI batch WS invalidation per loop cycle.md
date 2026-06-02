---
type: LLR
tags:
  - req/llr
  - tui
  - domain/event-loop
---
`App::on_ws_envelopes(iter)` MUST collect the union of cache-key prefixes returned by `ws_invalidation_prefixes(env.kind, env.entity)` across every envelope in the batch into a `HashSet`, then invoke `QueryCache::invalidate(prefix)` exactly once per unique prefix. When at least one envelope is `WsKind::Span | Derived | Trace`, the app MUST dispatch `Scenario::on_ws_batch` to every scenario with `WsBatchMeta { touches_spans: true }` and refresh the Context Growth Widget's `last_visible_rows` count. Envelope payloads MUST NOT be cloned into any per-envelope live-feed or `last_ws_event` field.

## Rationale
A burst of N envelopes collapses to one cache scan per dirtied prefix and one follow-mode advance, instead of N independent scans. Replaces the deleted `live_feed` ring buffer and the per-frame coalescer task.

## Derived from
- [[Terminal Event Loop]]
- [[Async Query Cache]]
