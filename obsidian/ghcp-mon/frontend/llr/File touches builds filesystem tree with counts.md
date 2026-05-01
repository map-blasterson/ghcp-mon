---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
For every matching tool span, `FileTouchesScenario` MUST fetch its detail under the shared `["span", trace_id, span_id]` query key, MUST extract the `path` argument (skipping spans with no string `path`), MUST split the path on `/` (collapsing repeated separators and trimming trailing `/`), and MUST insert a `Touch` along that path that increments every ancestor directory's `reads` or `writes` counter and appends to the leaf node's `fileTouches`.

## Rationale
Builds an aggregate filesystem view from per-call touches; the shared query key reuses the cache populated by ToolDetail and InputBreakdown.

## Derived from
- [[File Touch Tree]]
