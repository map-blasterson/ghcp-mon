---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose normalized tool kind is `read` (resolution per the active vendor adapter, selected per-span by `service_name`), `ToolDetailScenario` MUST resolve the file body from the normalized result envelope by using `body_string` when present, otherwise `output_text`; MUST apply the active vendor's read-body envelope unwrap to that body (no-op when the vendor declares none); MUST strip leading line-number prefixes matching `/^(\d+)[.:]\s(.*)$/` from each line, placing the captured numbers into a separate `<pre class="lns">` gutter; and MUST render the stripped body via `CodeBlock` with `langFromPath(<normalized file-path argument>)`. When no line carried a numbered prefix the body MUST be rendered without a gutter.

## Rationale
Prism cannot syntax-highlight content prefixed with literal line numbers; the gutter restores the visual numbering. The vendor adapter is responsible for any envelope unwrap required before line-prefix stripping (today, [[opencode read body XML envelope unwrap]]); the single regex covers both `"N. "` and `"N: "` line-prefix styles.

## Derived from
- [[Tool Call Inspector]]
