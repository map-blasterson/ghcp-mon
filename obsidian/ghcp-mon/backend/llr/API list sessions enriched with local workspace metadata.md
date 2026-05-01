---
type: LLR
tags:
  - req/llr
  - domain/api
  - domain/local-session
---
For each row returned by `GET /api/sessions`, the response MUST include `local_name`, `user_named`, `cwd`, and `branch` fields populated from `workspace.yaml` read via `local_session::read_workspace_yaml(resolve_session_state_dir(state.session_state_dir_override), conversation_id)`; when the file is missing or unreadable, all four fields MUST be `null` and the row MUST still be returned.

## Rationale
Sessions display human-readable names that only exist in the CLI's local sidecar.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]
- [[Local Session Metadata]]

## Test case
- [[REST API Tests]]
