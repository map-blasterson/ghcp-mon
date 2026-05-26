---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For the `edit` and `write` tool detail panels, `renderEditResult` SHALL choose its result renderer in this order: (1) a string result MUST be rendered verbatim via a searchable `TextBlock`; (2) an object result whose `metadata.diff` is a non-empty string MUST be rendered as a colored unified-diff `<pre class="edit-diff">` in which each line is wrapped in a `<span>` whose class is `udiff-meta` for lines starting with `+++`/`---`/`Index:`/`====`, `udiff-hunk` for lines starting with `@@`, `udiff-add` for `+`-prefixed lines (other than `+++`), `udiff-rem` for `-`-prefixed lines (other than `---`), and bare `udiff-line` otherwise; (3) an object result with a non-empty string `output` (and no `metadata.diff`) MUST be rendered as plain text; (4) anything else MUST fall back to `prettyJson` rendered as a JSON block.

## Rationale
opencode's `edit` result is `{title, output, metadata: {diff, filediff, diagnostics}}` — the unified-diff body in `metadata.diff` is the most informative payload and should drive the renderer rather than the full envelope. opencode's `write` result is `{title, output: "Wrote file successfully.", metadata}` — its `output` is the only useful string. Copilot results are plain strings (or structured JSON without these envelopes) and must pass through unchanged.

## Derived from
- [[Edit tool renders old new with syntax highlight]]
- [[Tool Call Inspector]]
