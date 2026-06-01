---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
The TUI tool-detail column MUST compose its body as a single flat, vertically-scrolled list of styled `Line`s, rendered top→bottom in this order: a collapsible metadata panel, an optional hero panel, the `args / result` section produced by the dispatched renderer, and a collapsible `raw span attributes` JSON panel. Long blobs (args, results, code, JSON) MUST each be wrapped in a searchable body block so the per-block `/` search affordance and the span-level `external_query` highlight apply uniformly. Column scroll (`↑`/`↓`/`Home`/`End`) MUST clamp to `body_len - view_h` and bring the focused active match (or focused block) into view.

## Rationale
A single line list with column-level scroll keeps the renderers composable and lets one paint pass unify syntect/diff base styling with the search-match overlay, mirroring the web `ToolDetailBody` section stack within terminal-cell constraints.

## Derived from
- [[Tool detail body blocks wrap in TextBlock for search]]
- [[Terminal Rendering Constraints]]
