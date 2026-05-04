---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When search results are active, the Spans column MUST add a `search-hit` CSS class (subtle background highlight) to every span tree row whose `span_id` appears in the search response, and MUST add a `search-miss` class to all other rows that applies `filter: saturate(0)` and reduced `opacity` so that text, kind badges, and tags all appear gray and faded. Clicking a highlighted result MUST propagate selection via the existing `onPickSpan` mechanism.

## Rationale
Full desaturation (not just text-color dimming) ensures that colored badges and tags on non-matching rows do not compete for attention with actual search hits. Keeping all rows visible preserves tree context and scroll position.

## Derived from
- [[Trace and Span Explorer]]
