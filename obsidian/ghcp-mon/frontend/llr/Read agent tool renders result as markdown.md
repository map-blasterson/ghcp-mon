---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose `tool_name === "read_agent"`, `ToolDetailScenario` MUST render the arguments as a plain key/value list and, when the result is a string, MUST render it as Markdown via `react-markdown` with `remark-gfm`; non-string results MUST fall back to pretty-printed JSON.

## Rationale
`read_agent` returns a sub-agent transcript that is itself Markdown; non-string results indicate a structured payload best shown verbatim.

## Derived from
- [[Tool Call Inspector]]
