---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose `tool_name` is `"view"` or `"read"`, `ToolDetailScenario` MUST resolve the file body from the result by (a) using the result verbatim when it is a string, or (b) reading `result.output` when the result is an object; MUST then unwrap a `<path>…</path><type>…</type><content>…</content>` XML envelope from the body when present (opencode `read` wrap); MUST strip leading line-number prefixes matching `/^(\d+)[.:]\s(.*)$/` (handling both Copilot `view`'s `"N. "` and opencode `read`'s `"N: "` styles) from each line, placing the captured numbers into a separate `<pre class="lns">` gutter, and rendering the stripped body via `CodeBlock` with `langFromPath(args.path ?? args.filePath)`. When no line carried a numbered prefix the body MUST be rendered without a gutter.

## Rationale
Prism cannot syntax-highlight content prefixed with literal line numbers; the gutter restores the visual numbering. opencode `read` returns the body inside a JSON envelope wrapped in XML tags and uses a different prefix delimiter, so both shapes must be unwrapped before highlighting.

## Derived from
- [[Tool Call Inspector]]
