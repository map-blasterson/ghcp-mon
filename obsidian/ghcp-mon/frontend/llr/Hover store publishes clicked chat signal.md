---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
`useHoverState` SHALL expose a `clickedChat: { traceId: string; spanId: string } | null` field with a `setClickedChat` setter. This value SHALL act as a fire-and-forget signal: the producer (Context Growth Widget bar click) sets it, and the consumer (SpansScenario) reads, acts on, and immediately clears it to `null`. The field MUST NOT be persisted across reloads.

## Rationale
Clicking a bar in the context growth chart needs to drive span selection in the Spans column without tight coupling between the two components. A transient Zustand field provides a simple pub/sub channel that any number of consumers can observe.

## Derived from
- [[Context Growth Widget]]
