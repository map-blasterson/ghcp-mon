---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-sessions
---
Each row in the TUI `LiveSessions` column MUST render in this cell order:
(1) first 8 chars of `conversation_id` (monospaced); (2) two-cell gap;
(3) `fmt_relative(last_seen_ns)`; (4) two-cell gap; (5) `latest_model`
(or `—` when None); (6) two-cell gap; (7) `chat_turn_count` / `tool_call_count`
/ `agent_run_count` with singular/plural suffixes, separated by ` / `.

The focused row receives a cyan background + bold-black foreground
highlight across the full row.

## Rationale
A fixed cell layout makes the session list scannable at a glance and
mirrors the web UI's column order (id, when, model, counts).

## Derived from
- [[Live sessions list summary stats]]
