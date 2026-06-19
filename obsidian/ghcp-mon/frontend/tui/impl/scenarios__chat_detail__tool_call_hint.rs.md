---
type: impl
source: src/tui/scenarios/chat_detail/tool_call_hint.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
`auto_expand_for_tool_call(root, tool_call_id)` locates the `tool`-role input message whose `tool_call_response` part has `id == tool_call_id` (only inspecting the `root/input` subtree). On a hit it returns `(ancestor_set, target_id)` where the ancestor set is `{root, root/input, root/input/input_messages, <matching message id>}` and `target_id` is the matching message's NodeId. The renderer unions the ancestor set into `expanded` non-destructively (preserving user-opened nodes) and paints `▶` in the arrow gutter at the row of `target_id`. Returns `None` when the tool_call_id is empty or no matching message exists.

## Source For
- [[Chat detail tool-call hint auto-expand and arrow]]
- [[TUI Chat detail tool-call arrow gutter]]
