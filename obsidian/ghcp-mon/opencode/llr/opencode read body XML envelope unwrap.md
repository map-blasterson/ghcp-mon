---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/opencode
  - domain/tool-detail
---
When `service_name === "opencode"` and a read-tool result body begins with `<path>…</path><type>…</type><content>…</content>`, the opencode read-body envelope unwrap SHALL extract the `<content>` element's text as the body fed to subsequent rendering. When the envelope is absent or malformed, the body MUST be passed through unchanged.

## Rationale
opencode `read` returns the file body inside a JSON envelope wrapped in XML tags; unwrapping it lets the shared View-tool renderer treat it as plain numbered text.

## Derived from
- [[opencode tool-call shape]]
- [[View tool splits line numbers into gutter]]
