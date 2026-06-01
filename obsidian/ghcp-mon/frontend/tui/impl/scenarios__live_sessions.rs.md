---
type: impl
source: src/tui/scenarios/live_sessions.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
LiveSessions scenario: list summary rows (first-8-char cid + relative time + model + plural-aware counts); `propagate_session` writes `config.session` to origin + every `spans`/`chat_detail`/`file_touches` column; `clear_session_everywhere` removes `session` (and dependent selected_* keys) from every column whose value equals the deleted cid; `delete_prompt` builds the confirm copy. Live-feed invalidation is handled by Phase 0's cache table: `(derived, session)` and `(derived, chat_turn)` envelopes invalidate `["sessions"]`, and the App's `cached_sessions` triggers a refetch on next render via `cache_get`.

## Source For
- [[Live sessions list summary stats]]
- [[Selecting session propagates to dependent columns]]
- [[Delete session confirms and clears column session]]
- [[Live sessions invalidation on session and chat turn events]]
- [[TUI Live sessions row layout in cells]]
