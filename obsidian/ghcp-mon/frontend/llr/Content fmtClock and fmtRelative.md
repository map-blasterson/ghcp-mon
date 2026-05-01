---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
`fmtClock(ns)` MUST return `"—"` when `ns == null` and otherwise `new Date(ns / 1_000_000).toLocaleTimeString()`. `fmtRelative(ns)` MUST return `"—"` when null, `"now"` when the elapsed millisecond delta is negative or below 1000, `"<n>s ago"` for `< 60_000`, `"<n>m ago"` for `< 3_600_000`, `"<n>h ago"` for `< 86_400_000`, and `"<n>d ago"` otherwise.

## Rationale
Two complementary surfaces — wall-clock and relative — used by row timestamps across the dashboard.

## Derived from
- [[Chat detail]]
