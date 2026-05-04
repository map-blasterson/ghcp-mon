---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
When the user clicks a bar in the Context Growth Widget chart, the widget SHALL call `setClickedChat({ traceId, spanId })` with the corresponding chat span's `trace_id` and `span_id`. The `SpansScenario` component SHALL consume the `clickedChat` signal, call `onPickSpan(traceId, spanId, "chat")` to drive span selection (and cross-column propagation), and then clear the signal by calling `setClickedChat(null)`.

## Rationale
Provides a direct navigation path from context usage to the chat span responsible, so users can jump from the aggregate view to the detailed conversation tree without manually finding the span.

## Derived from
- [[Context Growth Widget]]
