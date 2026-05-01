---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose `tool_name === "view"`, `ToolDetailScenario` MUST, when the result is a string, strip leading `"<n>. "` line-number prefixes from each result line into a separate `<pre class="lns">` gutter and render the stripped body via `CodeBlock` with `langFromPath(path)`; if no line had a numbered prefix it MUST render the body without a gutter.

## Rationale
Prism cannot syntax-highlight content prefixed with the literal `view` line numbers; the gutter restores the visual numbering.

## Derived from
- [[Tool Call Inspector]]
