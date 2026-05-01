---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
`parseToolCallResult(a)` MUST return `null` when `gen_ai.tool.call.result` is null/undefined; when it is a string it MUST attempt `JSON.parse` and return the parsed object/array, MUST return the raw string when the parse yields a string primitive, and MUST return the raw string verbatim when parsing throws.

## Rationale
Per spec note [5] this is opaque; bash-family tools return literal stdout/stderr that must not be re-encoded.

## Derived from
- [[Chat Input Breakdown]]
