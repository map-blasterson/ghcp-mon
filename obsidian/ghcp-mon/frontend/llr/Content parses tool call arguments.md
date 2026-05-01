---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
`parseToolCallArguments(a)` MUST return `null` when `gen_ai.tool.call.arguments` is null/undefined; when it is a string it MUST `JSON.parse` and return the parsed value, falling back to the verbatim string on parse failure; when it is already a non-string value it MUST return it unchanged.

## Rationale
Per execute-tool spec note [4] this attribute is a JSON-object string but real tools sometimes emit non-JSON.

## Derived from
- [[Chat Input Breakdown]]
