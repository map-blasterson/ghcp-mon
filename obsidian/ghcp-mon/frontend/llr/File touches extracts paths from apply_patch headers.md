---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
When the tool name is `"apply_patch"`, `extractApplyPatchPaths` SHALL parse the tool call's arguments (accepting either a raw string or an object with a `patch` or `input` string property) by scanning each line for the pattern `*** (Add|Update|Delete) File: <path>` or `Move to: <path>`, and SHALL return the set of distinct extracted path strings. If no matching headers are found or the argument is not a string, the function SHALL return an empty array.

## Rationale
`apply_patch` is Copilot's multi-file edit tool. Its arguments carry a unified-diff-like format where each file section is introduced by a header line. Extracting file paths from those headers lets the file-touch tree include patched files alongside view/edit/create tool calls.

## Derived from
- [[File Touch Tree]]
