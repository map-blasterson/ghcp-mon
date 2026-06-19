---
type: LLR
tags:
  - req/llr
  - tui
  - domain/workspace
---
`persist::save` MUST serialize the `Workspace` to TOML and write it to `dirs::cache_dir()/ghcp-mon/tui-workspace.toml`, creating the parent directory if missing. `persist::load` MUST read that file, run the migrate-drop step, and return the parsed workspace; absence or parse failure MUST yield the seeded default.

## Rationale
User-personalized layout survives quit/restart; corruption recovers gracefully.

## Derived from
- [[Workspace Persistence]]
- [[Workspace persists columns to localStorage]]
