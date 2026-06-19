---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
`JsonView::pretty(value)` MUST pretty-print a JSON value with 2-space indentation and MUST never panic (falling back to a safe rendering for any value). In the tool-detail column, JSON panels (the `raw span attributes` panel and any structured args/result block) MUST default to collapsed: a closed panel renders only a summary row prefixed `▸` and MUST NOT render the JSON body; `Space` on the focused JSON panel toggles it open (`▾`), revealing the searchable pretty-printed body.

## Rationale
Collapsing verbose JSON by default keeps the column scannable, matching the web `JsonView collapsed` default; the `▸`/`▾` glyph is the terminal analog of the HTML `<details>` disclosure triangle.

## Derived from
- [[JsonView pretty prints with optional collapse]]
