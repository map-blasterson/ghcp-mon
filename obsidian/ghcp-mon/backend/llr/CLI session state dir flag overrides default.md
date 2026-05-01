---
type: LLR
tags:
  - req/llr
  - domain/cli
  - domain/local-session
---
The CLI MUST accept a global `--session-state-dir <PATH>` flag (clap, `global = true`, optional) that overrides the base directory used to read per-conversation `workspace.yaml` sidecars. The parsed value MUST be threaded into `AppState.session_state_dir_override` (an `Arc<Option<PathBuf>>`) for both the `Serve` and `Replay --inline` subcommands, and API handlers MUST resolve the effective directory by passing that override into `local_session::resolve_session_state_dir`.

## Rationale
Operators and tests need to point the dashboard at an alternate session-state tree without setting an environment variable; threading the value through `AppState` keeps the resolution function pure and testable.

## Test context
- [[CLI Main Cheatsheet]]

## Derived from
- [[CLI Entry Point]]
- [[Local Session Metadata]]

## Test case
- [[CLI Main Tests]]
