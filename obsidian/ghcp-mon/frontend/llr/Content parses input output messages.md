---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
`parseInputMessages(a)` and `parseOutputMessages(a)` MUST accept either an already-parsed array or a JSON-stringified array stored at `gen_ai.input.messages` / `gen_ai.output.messages`, MUST iterate the array and produce one `Message` per entry whose `role` is the entry's string `role` (or `"unknown"`), whose `parts` is the entry's normalized `parts` array, and whose optional `finish_reason` is copied through when it is a string.

## Rationale
The wire payloads are JSON strings per the OTel GenAI schemas; the parser tolerates both shapes for testing convenience.

## Derived from
- [[Chat Input Breakdown]]
