---
type: impl
source: src/tui/scenarios/chat_detail/diff_segments.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Word-level diff over `prior` vs `current` strings via `similar::TextDiff::from_words`. `DiffSegment` enum (Unchanged/Added/Removed); `word_diff` produces a flat segment list, merging consecutive same-tag runs into a single segment. `count_added` / `count_removed` return total character counts per side (used to compose the `+X ch · -Y ch` meta). `all_unchanged` is the predicate `system_diff` consults to decide whether two non-equal `parts` arrays actually produced any visible word-level change.

## Source For
- [[Chat detail system instructions word-diff in DELTA]]
