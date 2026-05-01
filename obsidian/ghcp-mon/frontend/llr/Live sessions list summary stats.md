---
type: LLR
tags:
  - req/llr
  - domain/live-sessions
---
The `LiveSessionsScenario` MUST query `api.listSessions({ limit: 50 })` and render one row per `SessionSummary`, displaying for each session: a name (the `local_name` if non-empty else the first 8 chars of `conversation_id` rendered monospaced), an `fmtRelative(last_seen_ns)` timestamp, the `latest_model` (or `—`), and `chat_turn_count`/`tool_call_count`/`agent_run_count` with singular/plural suffixes; if `branch` is set it MUST render `branch` as a chip whose `title` is `cwd ?? undefined`.

## Rationale
This is the dashboard's primary "what's happening" surface; every shown field is sourced directly from the backend response.

## Derived from
- [[Live Session Browser]]
