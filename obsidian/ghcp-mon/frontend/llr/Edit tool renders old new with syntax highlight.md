---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose `tool_name` is `"edit"` or `"write"`, `ToolDetailScenario` MUST render the `path` argument (accepting `args.path` or, as fallback, `args.filePath`) as a key/value row, the `old_str` argument (accepting `args.old_str` or `args.oldString`) as a `CodeBlock` with class `edit-diff edit-diff-old`, the body argument as a `CodeBlock` with class `edit-diff edit-diff-new` (resolved from `args.new_str`, `args.newString`, or — for opencode `write` — `args.content`), all highlighted using `langFromPath(path)`, and any other arguments as JSON under an `other` label. The body's label MUST be `"new_str"` except when the value came from `args.content` (and neither `new_str` nor `newString` was a string), in which case the label MUST be `"content"`. `old_str` MUST be omitted when neither key is present (i.e., for new-file `write`).

## Rationale
Renders an inline diff-style preview so the user can see what changed without opening the file. opencode's `edit` tool uses `oldString`/`newString` and its `write` tool emits a single `content` body; relabeling the body keeps the panel honest about which source field the user is looking at.

## Derived from
- [[Tool Call Inspector]]
