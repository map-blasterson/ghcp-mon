---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For a tool span whose normalized tool kind is `edit` or `write` (resolution per the active vendor adapter, selected per-span by `service_name`), `ToolDetailScenario` MUST render the normalized file-path argument as a key/value row; the normalized old-text argument as a `CodeBlock` with class `edit-diff edit-diff-old` when present; a body `CodeBlock` with class `edit-diff edit-diff-new` whose value is sourced from the normalized new-text argument when present, otherwise from the normalized body-text argument; both code blocks highlighted using `langFromPath(<file-path>)`; and any other raw arguments dumped as JSON under an `other` label. The body block's label MUST be `"new_str"` when sourced from the new-text argument and `"content"` when sourced from the body-text argument. The old-text block MUST be omitted when no old-text argument is present (i.e., for new-file writes).

## Rationale
Renders an inline diff-style preview so the user can see what changed without opening the file. The label distinguishes which normalized concept supplied the body so the panel stays honest about its provenance. Vendor-specific raw argument names are resolved by the vendor adapter LLRs (e.g., [[Copilot tool-call argument mapping]], [[opencode tool-call argument mapping]]).

## Derived from
- [[Tool Call Inspector]]
