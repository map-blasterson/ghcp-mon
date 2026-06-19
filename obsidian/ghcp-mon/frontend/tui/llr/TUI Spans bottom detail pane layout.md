---
type: LLR
tags:
  - req/llr
  - tui
  - domain/traces
---
The TUI `SpansScenario` renders the span detail inspector as a **split-pane at the bottom of the column** rather than as a popup overlay or a separate column.

Layout:
- When the column's inner body region is **≥ 12 rows tall**, the bottom 6 rows are reserved for the detail pane (separated from the tree by a single-line `Borders::TOP` with the title ` detail `). When the body is shorter, the detail pane is suppressed and the tree consumes the full body.
- Pane contents (top-to-bottom):
  1. **Header line** — `<name>  [<kind_label>]  <8-char-span-id>`, white bold.
  2. **Parent line** — `↑ parent: <parent.name> (<8-char>)` (cyan) or `↑ parent: —` when None.
  3. **Children block** — `↓ children (N):` (cyan) followed by up to `pane_height - 3` rows of `  • <name> [<kind>] <8-char>`.
  4. **Projection summary** (magenta) — `projection: chat_turn, tool_call, …` listing which `SpanProjection` sub-blocks are present in the cached detail. Suppressed when projection is empty.
  5. **`(loading detail…)`** italic dim-gray fallback when the per-span detail is still in flight (parent/children fall back to the in-tree node's data while loading).

The pane reads the focused row's detail from the shared
`["span", trace_id, span_id]` cache key (per
[[Span inspector fetches and renders detail]]). The cache populates via
`cache_get` with `stale_after = 30 s`.

## Rationale
Terminal real estate is precious; a bottom split is the conventional
"inspector" pattern (Helix, ranger, lazygit) and keeps the focused row +
its detail visible side-by-side without consuming a whole separate column.
Suppressing the pane below 12 rows preserves usability at small terminal
sizes.

## Derived from
- [[Span detail view shows parent and children]]
- [[Span detail view renders projection sub-blocks]]
- [[Span inspector fetches and renders detail]]
- [[Terminal Rendering Constraints]]
