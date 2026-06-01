---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/keymap
---
Crossterm's mouse capture interferes with terminal text selection and scrollback, so it is disabled by default. `ghcp-mon attach --mouse` enables it at startup; the `M` key toggles it at runtime. All scenarios provide a complete keyboard path — mouse events are additive (hover ≅ focused-row position; click ≅ `Enter`).

## Derived LLRs
- [[TUI mouse capture opt-in via flag and runtime toggle]]
