---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/copilot
  - domain/file-touches
---
When the tool name is `"apply_patch"`, `extractApplyPatchPaths` SHALL parse the tool call's arguments (either a raw string or an object with a `patch` or `input` string property) and SHALL return the set of distinct paths extracted by scanning each line for `*** (Add|Update|Delete) File: <path>` or `Move to: <path>`. When no matching headers are found or the argument is not a string, it SHALL return an empty array.

## Rationale
`apply_patch` is Copilot's multi-file edit tool; its arguments carry a unified-diff-like format whose section headers identify the touched paths.

## Derived from
- [[Copilot tool-call shape]]
- [[File touches aggregates view edit create]]
- [[File Touch Tree]]
