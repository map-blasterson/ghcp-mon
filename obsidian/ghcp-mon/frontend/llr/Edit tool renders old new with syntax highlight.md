---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose `tool_name === "edit"`, `ToolDetailScenario` MUST render the `path` argument as a key/value row, the `old_str` argument as a `CodeBlock` with class `edit-diff edit-diff-old`, the `new_str` argument as a `CodeBlock` with class `edit-diff edit-diff-new`, both highlighted using the language returned by `langFromPath(path)`, and any other arguments as JSON under an `other` label.

## Rationale
Renders an inline diff-style preview of the edit so the user can see what changed without opening the file.

## Derived from
- [[Tool Call Inspector]]
