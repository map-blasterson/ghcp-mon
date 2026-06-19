---
type: impl
source: src/tui/scenarios/spans/follow_chat.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Pure function `find_following_chat_span(tree, picked_span_id)` implementing the 3-rule shape-aware dispatch — vendor-agnostic, does NOT consult `service_name`:
1. NESTED: picked has a chat-class ancestor → smallest chat sortKey strictly greater than nearest chat ancestor's sortKey, anywhere in tree.
2. SIBLING: no chat ancestor → first chat sibling of picked whose `[start, end]` encloses picked's `[start, end]`.
3. FALLBACK: next chat sibling chronologically (sortKey strictly greater than picked).
Returns `None` if none of the rules match; caller leaves selection unchanged.

## Source For
- [[Spans execute_tool selection auto-advances chat detail]]
