---
type: impl
source: src/tui/scenarios/spans/attrs.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Helpers for reading OTel / Copilot tool-call attributes out of a `SpanFull.attributes` map. `parse_tool_call_arguments` accepts either an inline object/array under `gen_ai.tool.call.arguments` or a stringified JSON blob (mirrors the web `parseToolCallArguments`); malformed strings yield `Value::Null` rather than panicking. Used by the chips renderer in `app.rs`.

## Source For
- [[Content parses tool call arguments]]
- [[Spans diff stat badges on file mutation tools]]
