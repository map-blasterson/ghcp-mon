---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/copilot
  - domain/live-sessions
---
When `service_name === "github-copilot"`, `LiveSessionsScenario` SHALL specialize the session row over its baseline composition by additionally rendering, on top of the vendor-agnostic fields (`conversation_id`, `latest_model`, `chat_turn_count`, `tool_call_count`, `agent_run_count`, `last_seen_ns`): the `local_name` display name as the row's primary label when `local_name` is a non-empty string (with a trailing 8-character monospaced `conversation_id`-prefix chip after it); the `auto` badge when the session was auto-summarized (`user_named === false`); and the `branch` chip (with `title = cwd ?? undefined`) when `branch` is a non-empty string. When `local_name` is absent or empty, the row's primary label MUST fall back to the 8-character monospaced `conversation_id` prefix. Each of these elements MUST be omitted for spans whose `service_name` does not equal `"github-copilot"`.

## Rationale
`local_name`, `user_named`, `cwd`, and `branch` come from the Copilot CLI's `~/.copilot/session-state/<cid>/workspace.yaml` sidecar and only make sense for Copilot sessions. Specializing the row positively (adding fields when the adapter is active) keeps the row visually honest when sessions from other OTLP producers are mixed into the list — they cleanly fall through to the baseline composition.

## Derived from
- [[Copilot session metadata]]
- [[Live Session Browser]]
