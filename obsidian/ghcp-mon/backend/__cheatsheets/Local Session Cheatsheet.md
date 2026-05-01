---
type: cheatsheet
---
Source: `src/local_session.rs`. Crate path: `ghcp_mon::local_session`.

## Extract

```rust
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceYaml {
    pub id: Option<String>,
    pub name: Option<String>,
    pub user_named: Option<bool>,
    pub summary: Option<String>,
    pub cwd: Option<String>,
    pub git_root: Option<String>,
    pub branch: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

pub fn resolve_session_state_dir(override_dir: Option<&Path>) -> Option<PathBuf>;
pub fn default_session_state_dir() -> Option<PathBuf>; // = resolve_session_state_dir(None)
pub fn read_workspace_yaml(base: &Path, cid: &str) -> Option<WorkspaceYaml>;
```

Resolution precedence used by `resolve_session_state_dir`:
1. `override_dir.map(|p| p.to_path_buf())` (CLI `--session-state-dir`).
2. `$COPILOT_SESSION_STATE_DIR` if set and non-empty.
3. `$HOME/.copilot/session-state` if `$HOME` is set and non-empty.
4. Otherwise `None`.

`read_workspace_yaml` rejects (returns `None`) when:
- `cid` is empty.
- `cid` contains `/`, `\`, or `..` (path-traversal defense).

Otherwise it reads `<base>/<cid>/workspace.yaml` with `std::fs::read`, parsing via `serde_yaml_ng::from_slice::<WorkspaceYaml>`. Any I/O or parse error → `None`.

YAML field names match struct field names verbatim (`name`, `user_named`, `branch`, `cwd`, etc.).

## Suggested Test Strategy

- Sync code, plain `#[test]`. The source already ships some inline tests; new cases should live in the same module pattern OR in `tests/` against `ghcp_mon::local_session`.
- For env-driven tests, **be aware that `std::env::set_var` is process-global and not safe under parallel test execution**. Use `--test-threads=1`, or scope env mutations to a single serialized test (the `serial_test` crate is not in dev-deps; a simple `Mutex` plus `set_var/remove_var` works). Save/restore prior values.
- For `resolve_session_state_dir`:
  - flag override returns its argument unchanged (as `PathBuf`).
  - env override only when flag is `None` and env var is non-empty.
  - $HOME fallback yields `<HOME>/.copilot/session-state`.
  - missing $HOME (and missing env, no flag) → `None`.
- For `read_workspace_yaml`:
  - Build a temp dir manually (mirror `tempdir_unique` from the inline tests).
  - Write a known YAML file under `<tmp>/<cid>/workspace.yaml`. Assert returned `WorkspaceYaml.name`, `branch`, `user_named` etc.
  - Path-traversal cases: `"../etc"`, `"a/b"`, `"a\\b"`, `".."`, `""` all return `None` without touching the filesystem (no need to create files).
  - Missing-dir / unreadable-file → `None`. Bad YAML → `None`.
- No mocks needed; the only seam is the filesystem and we control it.
