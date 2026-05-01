---
type: LLR
tags:
  - req/llr
  - domain/local-session
---
For valid `cid`, `local_session::read_workspace_yaml(base, cid)` MUST attempt to read `<base>/<cid>/workspace.yaml` and parse it as a `WorkspaceYaml` struct (fields: `id`, `name`, `user_named`, `summary`, `cwd`, `git_root`, `branch`, `created_at`, `updated_at` — all optional), returning `Some(WorkspaceYaml)` on success and `None` on any I/O or parse error.

## Rationale
The sidecar file is owned by the Copilot CLI; the dashboard must never fail because a session has no metadata yet.

## Test context
- [[Local Session Cheatsheet]]

## Derived from
- [[Local Session Metadata]]

## Test case
- [[Local Session Tests]]
