---
type: LLR
tags:
  - req/llr
  - domain/api
  - domain/local-session
---
The body returned by `GET /api/sessions/:cid` MUST include `local_name`, `user_named`, `cwd`, and `branch` fields populated from `workspace.yaml` read via `local_session::read_workspace_yaml(resolve_session_state_dir(state.session_state_dir_override), :cid)`; when no sidecar metadata is available, all four MUST be `null`.

## Rationale
The session detail view needs the same human-readable metadata as the list to keep the UI consistent.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]
- [[Local Session Metadata]]

## Test case
- [[REST API Tests]]
