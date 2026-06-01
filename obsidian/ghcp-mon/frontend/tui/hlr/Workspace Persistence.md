---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/workspace
---
The workspace (columns, context-widget state) is persisted to `dirs::cache_dir()/ghcp-mon/tui-workspace.toml` and reloaded on startup. The persisted document carries a `schema_version` integer; on load a migrate step drops any persisted column whose `scenario_type` is in the obsolete set (`{context_growth, tool_registry, context_inspector, shell_io}`). Parse failure or absence yields the seeded default (Sessions / Spans / Tool detail / Chat detail).

## Derived LLRs
- [[TUI workspace persistence to cache dir TOML]]
- [[TUI workspace migrate drops obsolete scenario types]]
- [[TUI workspace defaults seed four columns]]
