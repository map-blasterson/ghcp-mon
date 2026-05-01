---
type: LLR
tags:
  - req/llr
  - domain/local-session
---
`local_session::resolve_session_state_dir(override_dir)` MUST resolve the session-state base directory in the following precedence order: (1) when `override_dir` is `Some(path)` (set by the `--session-state-dir` CLI flag), it MUST return `Some(path.to_path_buf())`; (2) otherwise, when `$COPILOT_SESSION_STATE_DIR` is set and non-empty, it MUST return `Some(PathBuf::from($COPILOT_SESSION_STATE_DIR))`; (3) otherwise, when `$HOME` is set and non-empty, it MUST return `Some($HOME/.copilot/session-state)`; (4) otherwise it MUST return `None`. `default_session_state_dir()` MUST be equivalent to `resolve_session_state_dir(None)`.

## Rationale
The CLI flag / env var override exists so tests and non-default installations can point the dashboard at an alternate session-state tree without code changes; the explicit flag wins so per-invocation overrides are deterministic.

## Test context
- [[Local Session Cheatsheet]]

## Derived from
- [[Local Session Metadata]]

## Test case
- [[Local Session Tests]]
