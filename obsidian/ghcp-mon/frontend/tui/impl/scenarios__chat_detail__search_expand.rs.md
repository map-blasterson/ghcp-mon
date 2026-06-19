---
type: impl
source: src/tui/scenarios/chat_detail/search_expand.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Pure walker that, given a non-empty `query`, returns the set of node IDs that must be added to `expanded` to make every matching content visible. Match definition is mode-dependent per the LLR: FULL mode matches any content (labels, meta, primitive key/values, message-part bodies, all diff segments); DELTA mode for `SystemDiff` nodes ONLY matches `DiffSegment::Added` text (Unchanged/Removed are not "active" content in DELTA). For each match, every ancestor in the depth-first walk is added to the result; the matching node itself is not (its ancestors suffice to surface it). The renderer reconciles this set with the user-driven `expanded` set non-destructively (union into `expanded`, snapshot pre-search state into `search_expanded_snapshot` for restoration on clear).

## Source For
- [[ChatDetail auto-expands tree to span search matches]]
- [[TUI Chat detail search-expanded set tracks restoration]]
- [[TUI Chat detail auto-expand to search match content]]
