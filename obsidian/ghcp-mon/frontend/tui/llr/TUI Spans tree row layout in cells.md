---
type: LLR
tags:
  - req/llr
  - tui
  - domain/traces
---
Each span tree row in the TUI Spans column MUST render in this cell order:
(1) indent of `depth * 2` cells; (2) collapse glyph in 1 cell — `▾` when
expanded with children, `▸` when collapsed with children, ` ` (space) when
no children; (3) a 1-cell gap; (4) hash-coloured kind badge with bold black
text on the hash colour, padded to at most 10 cells (label per the
`Kind badge label renames raw kinds` map, seeded with the span name so
identical tool names share a colour); (5) a 1-cell gap; (6) optional
rolling-dots indicator when `ingestion_state === "placeholder"`; (7) the
span name, truncated with `…` when it would exceed the remaining column
width.

The focused row receives a cyan background highlight across the glyph and
name cells; non-matching search rows are dimmed (Style modifier `DIM`)
across the name cell; kind-filter non-matches are dimmed identically.

## Rationale
A single, fixed cell layout keeps the tree scannable at narrow column
widths and ensures the kind badge column aligns visually.

## Derived from
- [[Keybinding Matrix]]
- [[Terminal Rendering Constraints]]
