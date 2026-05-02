---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
While in the `active` phase with a non-empty `query`, after a 50 ms debounce `TextBlock` MUST wrap every case-insensitive occurrence of `query` in the rendered content in a `<mark class="tb-match">` element, set `matchCount` to the number of marks, mark the current one with the additional class `tb-match-current`, and MUST unwrap all `mark.tb-match` nodes (restoring the original text-node layout via `parent.normalize()`) on every re-run, on phase exit, and on effect cleanup.

## Rationale
Walking the DOM rather than re-rendering avoids breaking the host `<pre>` formatting; cleanup-on-cleanup prevents orphaned wrappers across re-renders.

## Derived from
- [[Searchable Text Block]]
