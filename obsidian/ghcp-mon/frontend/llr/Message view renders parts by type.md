---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
The `MessageView` component MUST render each `Message` with its `role` as a header and its `finish_reason` (when present) as a dim suffix; `PartView` MUST dispatch on `part.type`: `text` and `reasoning` render `content` in `<pre>`, `tool_call` renders `name`, short id, and pretty-printed `arguments`, `tool_call_response` renders short id and result (string verbatim or pretty-printed JSON), and any other `type` MUST render the part as JSON.

## Rationale
Per OTel GenAI part-type discriminator; unknown types are passed through to keep the view forward-compatible.

## Derived from
- [[Chat detail]]
