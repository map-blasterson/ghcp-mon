---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose tool kind is `edit` or `write`, `ToolDetailScenario` MUST render the file-path argument as a key/value row; the old-text argument as a `CodeBlock` with class `edit-diff edit-diff-old` when present; a body `CodeBlock` with class `edit-diff edit-diff-new` sourced from the new-text argument when present, otherwise from the body-text argument; both code blocks highlighted using `langFromPath(<file-path>)`; and any remaining raw arguments dumped as JSON under an `other` label. The body block's label MUST be `"new_str"` when sourced from new-text and `"content"` when sourced from body-text. The old-text block MUST be omitted when no old-text argument is present (i.e., for new-file writes).

## Rationale
Renders an inline diff-style preview so the user can see what changed without opening the file. The label distinguishes which concept supplied the body so the panel stays honest about its provenance.

## Derived from
- [[Tool Call Inspector]]
