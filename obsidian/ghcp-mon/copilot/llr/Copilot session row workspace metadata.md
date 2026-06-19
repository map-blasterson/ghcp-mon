---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/copilot
  - domain/live-sessions
---
When `service_name === "github-copilot"`, `LiveSessionsScenario` SHALL additionally render: `local_name` (when non-empty) as the row's primary label, followed by a trailing 8-character monospaced `conversation_id`-prefix chip; the `auto` badge (with title `"auto-summarized name (use /rename in copilot to set)"`) when `user_named === false`; and the `branch` chip (with `title = cwd ?? undefined`) when `branch` is non-empty.

## Rationale
`local_name`, `user_named`, `cwd`, and `branch` come from the Copilot CLI's `~/.copilot/session-state/<cid>/workspace.yaml` sidecar and only make sense for Copilot sessions.

## Derived from
- [[Copilot session metadata]]
- [[Live Session Browser]]
