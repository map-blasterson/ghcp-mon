---
type: LLR
tags:
  - req/llr
  - domain/live-sessions
---
The per-row delete button MUST `confirm()` with the prompt `"Delete session <8-char-id>? This removes all spans, turns, and tool calls in its trace(s)."`, MUST call `api.deleteSession(id)` only if the user confirms, MUST clear `config.session` on every column whose `config.session` equals the deleted id, and MUST invalidate the `["sessions"]` query.

## Rationale
Confirms a destructive cascade; clearing the session field stops dependent columns from showing 404s.

## Derived from
- [[Live Session Browser]]
