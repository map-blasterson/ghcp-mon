---
type: LLR
tags:
  - req/llr
  - domain/traces
---
The `SpanInspector` component MUST issue `useQuery({ queryKey: ["span", trace_id, span_id], queryFn: () => api.getSpan(trace_id, span_id) })`, MUST render `"loading…"` while pending, `"span not found"` on error or no data, and otherwise the `SpanDetailView` of the response.

## Rationale
The detail pane is the canonical inspection surface; the query key is shared with sibling scenarios so the cache hits across columns.

## Derived from
- [[Trace and Span Explorer]]
