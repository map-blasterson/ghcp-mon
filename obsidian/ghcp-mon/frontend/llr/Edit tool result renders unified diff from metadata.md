---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For tool spans whose tool kind is `edit` or `write`, `renderEditResult` SHALL select its result renderer from the result envelope in this precedence: (1) `body_string` present → render verbatim via a searchable `TextBlock`; (2) non-empty `diff_text` → render as a colored unified-diff `<pre class="edit-diff">` whose lines are classified by `udiffClassify` (one of `udiff-meta`, `udiff-hunk`, `udiff-add`, `udiff-rem`, or `udiff-line`); (3) non-empty `output_text` → render as plain text; (4) otherwise fall back to `prettyJson` as a JSON block.

## Rationale
The renderer prefers the most informative payload available in the result envelope, falling back progressively.

## Derived from
- [[Edit tool renders old new with syntax highlight]]
- [[Tool Call Inspector]]
