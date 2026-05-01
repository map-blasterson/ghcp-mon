---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/local_session.rs`

Covers LLRs:
- [[Local session state dir resolved from flag env or home]] — `resolve_flag_override_takes_precedence`, `resolve_uses_env_var_when_no_flag`, `resolve_falls_back_to_home_when_no_flag_no_env`, `resolve_returns_none_when_nothing_set`, `resolve_treats_empty_env_var_as_unset`, `default_session_state_dir_equivalent_to_resolve_none`.
- [[Local session workspace yaml best effort read]] — `read_workspace_yaml_parses_known_fields`, `read_workspace_yaml_returns_none_on_missing_file`, `read_workspace_yaml_returns_none_on_bad_yaml`.
- [[Local session workspace yaml rejects path traversal]] — `read_workspace_yaml_rejects_traversal_without_touching_fs`.

Tests serialize env-mutating operations on a `Mutex` and save/restore prior values per `EnvGuard`.
