---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/copilot
---
When `service_name === "github-copilot"`, the Copilot tool-call adapter SHALL populate the normalized result envelope by setting `body_string` to the raw string result whenever the parsed result value is a string, and MUST leave `output_text`, `diff_text`, and `metadata` unset. For non-string raw results (objects, arrays, null), `body_string` MUST also be left unset; all four envelope fields are then absent and downstream renderers fall back to their generic-JSON branch.

## Rationale
Copilot results travel as plain strings; the adapter exposes them as `body_string` so renderers (e.g., [[Edit tool result renders unified diff from metadata]], [[View tool splits line numbers into gutter]]) can branch on a single normalized field.

## Derived from
- [[Copilot tool-call shape]]
