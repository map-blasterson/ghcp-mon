---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
In `DELTA` mode, `buildSystemNode` MUST compare the current `systemParts` to `prior.systemParts` via deep-equal JSON: when equal it MUST emit a `system_unchanged` node (no children, meta `unchanged · N parts`); when different it MUST emit a `system` node carrying `badge: "CHANGED"` whose only child is a `system_diff` node whose `diffSegments` are produced by `diffWordsWithSpace(priorBody, currentBody)` over the concatenated text/reasoning content of each side, with `meta` reporting `+addedCh ch · -remCh ch`.

## Rationale
Word-granularity diff over concatenated bodies keeps the diff legible while preserving the structural placement under the `system instructions` node.

## Derived from
- [[Chat detail DELTA diffs against prior chat span]]
