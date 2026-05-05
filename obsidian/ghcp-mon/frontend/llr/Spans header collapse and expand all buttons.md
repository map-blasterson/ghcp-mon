---
type: LLR
tags:
  - req/llr
  - domain/traces
---
The Spans column header MUST render "Collapse all" (−) and "Expand all" (+) buttons that are disabled when no session is selected. "Collapse all" SHALL add every span with children to the `userCollapsed` set; "Expand all" SHALL clear the `userCollapsed` set entirely.

## Rationale
Gives users a one-click way to reduce tree clutter for large sessions or restore full visibility after collapsing.

## Derived from
- [[Trace and Span Explorer]]
