---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/opencode
---
When `service_name === "opencode"`, the opencode tool-call adapter SHALL populate the normalized result envelope as follows. For a string-typed raw result, `body_string` MUST be set to the string value and the other envelope fields MUST be left unset. For an object-typed raw result of shape `{title?, output?, metadata?}`: `diff_text` MUST be set to `metadata.diff` when that is a non-empty string (otherwise unset); `output_text` MUST be set to `output` when that is a non-empty string (otherwise unset); `metadata` MUST be set to the full raw `metadata` object when present (otherwise unset); `body_string` MUST be left unset. For any other raw result type (array, null, primitive non-string), all four envelope fields MUST be left unset.

## Rationale
opencode's `edit` result is `{title, output, metadata: {diff, filediff, diagnostics}}` and its `write` result is `{title, output: "Wrote file successfully.", metadata}`. Mapping these into the normalized envelope lets renderers (e.g., [[Edit tool result renders unified diff from metadata]]) branch on a single set of fields regardless of producer.

## Derived from
- [[opencode tool-call shape]]
