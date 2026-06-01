---
type: LLR
tags:
  - req/llr
  - tui
  - domain/traces
---
The Spans column's header occupies two rows above the body region.

- Row 1 renders the session label:
  - `session: <first-8-chars-of-cid>` when `column.config.session` is set.
  - `session: (none) — press 's'` when no session is selected.
- Row 2 renders a hint string for the kind filter, search box, follow-mode
  checkbox, and collapse/expand buttons, in left-to-right cell order:
  `k:kind  / <query>  [x|/ ] follow  +/-:expand/collapse  f:follow`.
  - When the search input is active (text-input mode), the query is
    suffixed with `_` as a cursor placeholder.
  - When follow-mode is on, the checkbox glyph is `[x]`; otherwise `[ ]`.

Body region rendered below the header consumes the remaining column
height.

## Rationale
Two rows separate the session selector (a stable identity) from the
filtering controls (mutable per-render), keeping the controls discoverable
at narrow column widths.

## Derived from
- [[Spans two-row header grid layout]]
- [[Terminal Rendering Constraints]]
