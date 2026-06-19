---
type: impl
source: src/tui/scenarios/chat_detail/messages.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Content helpers ported from web `MessageView`: `Message` (role/parts/finish_reason/raw), `Part` enum dispatching on the OTel GenAI `type` discriminator (`text`, `reasoning`, `tool_call`, `tool_call_response`, plus `Other { raw }` catch-all for forward compatibility). `attrs(span)` returns `span.attributes` as object, falling back to JSON-parse of `span.attributes_json`, falling back to empty object. `has_captured_content` checks any of the three content keys non-null. `parse_input_messages` / `parse_output_messages` accept inline array OR JSON-stringified array, defaulting `role` to `"unknown"`. `parse_tool_call_result` implements the three-way string/JSON/raw branching: null → `None`, string → JSON.parse (returning raw verbatim on string-primitive parse or parse failure), inline value → cloned through. `parse_part` dispatches; unknown discriminator falls through to `Other`.

## Source For
- [[Content attrs accepts object or json string]]
- [[Content parses input output messages]]
- [[Content parses tool call result]]
- [[Content has captured content predicate]]
- [[Message view renders parts by type]]
