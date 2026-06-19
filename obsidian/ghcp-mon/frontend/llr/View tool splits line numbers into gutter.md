---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose tool kind is `read`, `ToolDetailScenario` MUST resolve the file body from the result envelope by using `body_string` when present, otherwise `output_text`; MUST strip leading line-number prefixes matching `/^(\d+)[.:]\s(.*)$/` from each line, placing the captured numbers into a separate `<pre class="lns">` gutter; and MUST render the stripped body via `CodeBlock` with `langFromPath(<file-path>)`. When no line carried a numbered prefix the body MUST be rendered without a gutter.

## Rationale
Prism cannot syntax-highlight content prefixed with literal line numbers; the gutter restores the visual numbering. The single regex covers both `"N. "` and `"N: "` line-prefix styles.

## Derived from
- [[Tool Call Inspector]]
