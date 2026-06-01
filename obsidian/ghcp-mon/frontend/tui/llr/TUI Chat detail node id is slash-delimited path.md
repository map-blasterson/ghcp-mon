---
type: LLR
tags:
  - req/llr
  - tui
  - domain/input-breakdown
---
Every chat-detail tree node MUST be addressable by a stable slash-delimited string path (`NodeId(String)`) — `"root"`, `"root/system"`, `"root/input/input_messages/3"`, `"root/input/input_messages/3/parts/0"` — preserved verbatim across rebuilds of the tree. Primitive expand keys are addressed via a synthetic `"{node_id}__p{i}"` form (`i` = primitive index inside the node's `primitives` array) so per-node primitive expansion state survives tree rebuilds without any extra mapping table. The carried-forward suffix optimisation in [[Chat detail DELTA input messages carried-forward suffix]] MUST preserve the original-array index in suffix node ids (`root/input/input_messages/{prefix_len + j}`), keeping cross-links (the tool-call arrow target) stable irrespective of whether the optimisation fired.

## Rationale
String paths are cheap to compare and hash; preserving them across rebuilds is the simplest way to keep `expanded`, `expanded_prims`, and per-block search state attached to the same logical node when the tree is regenerated each frame.

## Derived from
- [[Chat detail DELTA input messages carried-forward suffix]]
- [[Chat detail tool-call hint auto-expand and arrow]]
