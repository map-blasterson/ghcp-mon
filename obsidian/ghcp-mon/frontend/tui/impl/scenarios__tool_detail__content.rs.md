---
type: impl
source: src/tui/scenarios/tool_detail/content.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
`parse_tool_call_result(attrs) -> Option<Value>` (port of the web `parseToolCallResult`: string that parses to a non-string value → parsed; otherwise raw string verbatim) plus `as_object`/`is_nullish` helpers. Complements the reused `spans::attrs::parse_tool_call_arguments`.

## Source For
- [[Content parses tool call result]]
- [[Generic tool renders args splitting code-ish strings]]
- [[Edit tool result renders unified diff from metadata]]
- [[View tool splits line numbers into gutter]]
