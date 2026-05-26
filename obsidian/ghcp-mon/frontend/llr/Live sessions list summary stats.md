---
type: LLR
tags:
  - req/llr
  - domain/live-sessions
  - vendor/copilot
---
The `LiveSessionsScenario` MUST query `api.listSessions({ limit: 50 })` and render one row per `SessionSummary` showing the first 8 chars of `conversation_id` (monospaced) as the row's primary label, an `fmtRelative(last_seen_ns)` timestamp, the `latest_model` (or `—`), and `chat_turn_count`/`tool_call_count`/`agent_run_count` with singular/plural suffixes. Vendor-specific row specializations are layered on top per [[Copilot session row workspace metadata]].

## Rationale
This is the dashboard's primary "what's happening" surface; every shown field is sourced directly from the backend response.

## Derived from
- [[Live Session Browser]]
