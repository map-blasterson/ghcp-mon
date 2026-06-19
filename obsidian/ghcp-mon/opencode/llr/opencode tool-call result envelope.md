---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/opencode
---
When `service_name === "opencode"`, the opencode tool-call adapter SHALL populate the normalized result envelope from the raw result as follows: a string result populates `body_string`; an object result of shape `{title?, output?, metadata?}` populates `diff_text` from `metadata.diff`, `output_text` from `output`, and `metadata` from `metadata` (each only when present, and the two strings only when non-empty); all other result shapes leave the envelope empty.

## Rationale
opencode's `edit` result is `{title, output, metadata: {diff, filediff, diagnostics}}` and its `write` result is `{title, output: "Wrote file successfully.", metadata}`. Mapping these into the normalized envelope lets renderers branch on a single set of fields.

## Derived from
- [[opencode tool-call shape]]
