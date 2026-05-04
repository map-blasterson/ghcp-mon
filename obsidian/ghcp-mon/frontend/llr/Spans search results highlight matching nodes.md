---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When search results are active, the Spans column MUST add a `search-hit` CSS class (subtle background highlight) to every span tree row whose `span_id` appears in the search response, and MUST add a `search-miss` class to all other rows that applies `filter: saturate(0)` and reduced `opacity` so that text, kind badges, and tags all appear gray and faded. Clicking a highlighted result MUST propagate selection via the existing `onPickSpan` mechanism. The search request MUST include a `mode` parameter derived from the sibling ChatDetail column's `chat_mode` config: when `chat_mode` is `DELTA`, the Spans column MUST pass `mode=delta` to the search API so that results exclude chat spans matching only in unchanged attributes. The React Query key MUST include the mode value so that toggling DELTA/FULL re-executes the search.

## Rationale
Full desaturation (not just text-color dimming) ensures that colored badges and tags on non-matching rows do not compete for attention with actual search hits. Keeping all rows visible preserves tree context and scroll position.

## Derived from
- [[Trace and Span Explorer]]
