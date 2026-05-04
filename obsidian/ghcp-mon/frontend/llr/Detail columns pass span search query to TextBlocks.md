---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
  - domain/tool-detail
---
`ChatDetailScenario` and `ToolDetailScenario` MUST read `column.config.search_query` and pass it as the `externalQuery` prop to every `TextBlock` component they render, so that TextBlock search highlighting activates automatically for the span-level search query. In ChatDetail's DELTA mode, highlights MUST only appear in active (added) diff segments; removed and unchanged segments MUST suppress highlight marks.

## Rationale
Connecting the span search query to TextBlock's existing highlighting avoids building separate highlight logic per detail column and gives the user a consistent visual treatment across all content areas.

## Derived from
- [[Chat detail]]
- [[Tool Call Inspector]]
