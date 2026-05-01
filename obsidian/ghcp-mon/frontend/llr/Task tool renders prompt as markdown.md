---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose `tool_name === "task"`, `ToolDetailScenario` MUST render the string `prompt` argument as Markdown via `react-markdown` with the `remark-gfm` plugin, and MUST render every other argument as a key/value row.

## Rationale
The `task` tool's prompt is authored as Markdown for the dispatched sub-agent; rendering it as such matches the source intent.

## Derived from
- [[Tool Call Inspector]]
