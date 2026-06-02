---
type: LLR
tags:
  - req/llr
  - tui
  - domain/chat-detail
---
`search_expand::search_expanded(root, query, mode)` MUST return the set of every node id whose ACTIVE content contains `query` case-insensitively, INCLUDING the matching node itself plus every ancestor on the path. In FULL mode "active" content is all content; in DELTA mode "active" content is restricted to `DiffSegment::Added`. Additionally, `search_expanded_prims(root, query)` MUST return `(node_id, primitive_index)` pairs whose key or value contains the needle, so long primitives auto-open from their truncated `(+)` state to reveal the match.

## Rationale
Previous behaviour expanded only ancestors, leaving the matching `Part::Text` row collapsed; users had to expand it manually to see the highlighted line.

## Derived from
- [[ChatDetail auto-expands tree to span search matches]]
- [[TUI Chat detail search-expanded set tracks restoration]]
