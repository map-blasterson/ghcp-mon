---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For tool spans whose normalized tool kind is `edit` or `write` (resolution per the active vendor adapter, selected per-span by `service_name`), `renderEditResult` SHALL select its result renderer from the normalized result envelope in the following precedence: (1) when the envelope exposes `body_string`, render it verbatim via a searchable `TextBlock`; (2) otherwise, when the envelope exposes a non-empty `diff_text`, render it as a colored unified-diff `<pre class="edit-diff">` in which each line is wrapped in a `<span>` whose class is `udiff-meta` for lines starting with `+++`/`---`/`Index:`/`====`, `udiff-hunk` for lines starting with `@@`, `udiff-add` for `+`-prefixed lines (other than `+++`), `udiff-rem` for `-`-prefixed lines (other than `---`), and bare `udiff-line` otherwise; (3) otherwise, when the envelope exposes a non-empty `output_text`, render it as plain text; (4) otherwise, fall back to `prettyJson` rendered as a JSON block.

## Rationale
The renderer prefers the most informative payload available in the normalized result envelope, falling back progressively. Which envelope field a given producer fills is the adapter's concern, not the renderer's (see [[Copilot tool-call result envelope]], [[opencode tool-call result envelope]]).

## Derived from
- [[Edit tool renders old new with syntax highlight]]
- [[Tool Call Inspector]]
