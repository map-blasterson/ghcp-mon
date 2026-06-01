---
type: impl
source: src/tui/widgets/context_growth/merge.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Pure merge logic for the Context Growth Widget: `chat_span_pks` (chat-span membership set, including invoke_agent sub-agent descendants), `merge_snapshots` (per-`span_pk` fold — max non-null `token_limit`, latest-non-null-by-`captured_ns` input/output/reasoning/cache_read with back-fill, sorted ascending by chat start with span_pk tie-break, `is_sub_agent` from invoke_agent ancestor depth), `max_current_tokens` (y-axis occupancy anchor), and `split_segments`.

## Source For
- [[Context widget merges chat span snapshots per span_pk]]
- [[Context widget includes sub-agent chats]]
- [[Context widget stack chart per turn]]
