---
type: LLR
tags:
  - req/llr
  - domain/traces
---
The `SpansScenario` column header MUST use a two-row CSS grid layout (`grid-template-columns: auto 1fr`). The first row SHALL contain a "session" label and session `<select>`. The second row SHALL contain a "kind" label followed by the kind-filter `<select>`, optional search input, follow-mode checkbox, and collapse/expand buttons arranged in a flex container.

## Rationale
Separating session selection from the kind/search/follow controls into distinct rows keeps the header readable at narrow column widths while still exposing all filter controls without scrolling.

## Derived from
- [[Trace and Span Explorer]]
