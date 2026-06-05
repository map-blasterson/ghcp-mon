---
type: HLR
tags:
  - req/hlr
  - domain/traces
---
The dashboard renders the trace/span hierarchy for a selected session as an expandable tree, falls back to a recent-traces list when no session is picked, and routes the selection to linked columns based on span kind so the user can inspect a span in a sibling detail column.

## Normalized tool-call vocabulary
Span-row LLRs that branch on tool-call data are specified over a normalized tool-call shape rather than raw OTLP fields. **Normalized tool kinds** include `read`, `write`, `edit`, `patch`, and `shell`. **Normalized arguments** are referred to as: *file-path*, *old-text*, *new-text*, *body-text*, *shell-command*, *target-url*, *patch-text*. The active **vendor adapter** is selected per-span by `service_name`; concrete mappings live in each vendor scope (e.g., [[Copilot tool-call shape]], [[opencode tool-call shape]]).

## Derived LLRs
- [[Spans scenario two modes session vs traces]]
- [[Spans live invalidation on ingest events]]
- [[Span selection routes by kind class allow list]]
- [[Spans follows latest tool span]]
- [[Spans execute_tool selection auto-advances chat detail]]
- [[Spans direct chat selection clears tool call hint]]
- [[Traces list dims rows below kind filter]]
- [[Span tree row publishes hovered chat ancestor]]
- [[Shell command chip extracts primary words]]
- [[Skill name chip shows skill argument]]
- [[Report intent title shows on parent row]]
- [[Span inspector fetches and renders detail]]
- [[Span detail view renders projection sub-blocks]]
- [[Span detail view shows parent and children]]
- [[Span detail view renders error block]]
- [[Span tree row shows error indicator]]
- [[Placeholder ingestion state shown with rolling dots]]
- [[Kind badge label renames raw kinds]]
- [[Hash color stable hue via FNV-1a]]
- [[Spans searchbox queries server on input]]
- [[Spans search results highlight matching nodes]]
- [[Spans search propagates query to detail columns]]
- [[Spans nodeMap provides O1 span lookup]]
- [[Spans invoke_agent selection routes to latest chat descendant]]
- [[Span tree kind-filter dims non-matching rows]]
- [[Spans header collapse and expand all buttons]]
- [[Spans two-row header grid layout]]
- [[Spans batch arrival smoothing]]
- [[Spans diff stat badges on file mutation tools]]
- [[Spans tool description inline label]]
