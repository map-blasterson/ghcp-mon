---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/opencode
  - domain/tool-detail
---
When `service_name === "opencode"` and the normalized read-tool result body (i.e., `body_string` if present else `output_text`, per [[opencode tool-call result envelope]]) contains a leading `<path>…</path><type>…</type><content>…</content>` XML envelope, the opencode read-body envelope unwrap SHALL extract the `<content>` element's text and substitute it as the body fed to subsequent rendering (line-prefix stripping and gutter assembly). When the envelope is absent or malformed, the body MUST be passed through unchanged.

## Rationale
opencode `read` returns the file body inside a JSON envelope wrapped in XML tags. Unwrapping it before line-prefix stripping lets the shared View-tool renderer treat the body as plain numbered text.

## Derived from
- [[opencode tool-call shape]]
- [[View tool splits line numbers into gutter]]
