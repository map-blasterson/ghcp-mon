---
type: LLR
tags:
  - req/llr
  - domain/live-sessions
  - github-specific
---
In `LiveSessionsScenario`, GitHub-Copilot-specific session-row UI elements MUST be gated by the predicate `isGhcp = (s.service_name === "github-copilot")`. Specifically: the `local_name` display name and the trailing short-id chip MUST only render when `isGhcp` is true and `local_name` is non-empty (otherwise the row falls back to the 8-character monospaced `conversation_id` prefix as its primary label); the `auto` badge for auto-summarized names MUST only render when `isGhcp` is true; the `branch` chip (with `title = cwd ?? undefined`) MUST only render when `isGhcp` is true and `branch` is set. For sessions reported by other vendors (any other `service_name` value, or `null`), the row MUST omit all of the above and rely on the vendor-agnostic fields (`conversation_id`, `latest_model`, `chat_turn_count`, `tool_call_count`, `agent_run_count`, `last_seen_ns`).

## Rationale
`local_name`, `user_named`, `cwd`, and `branch` come from the Copilot CLI's `~/.copilot/session-state/<cid>/workspace.yaml` sidecar and only make sense for Copilot sessions. Gating on `service_name` keeps the row visually honest when sessions from other OTLP producers are mixed into the list — they would otherwise render meaningless empty workspace metadata or, worse, misleading `auto` badges for fields they never opted into.

## Derived from
- [[Live Session Browser]]
