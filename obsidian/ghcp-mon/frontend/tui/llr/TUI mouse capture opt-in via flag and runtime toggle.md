---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
Mouse capture MUST default to disabled. `ghcp-mon attach --mouse` MUST emit `EnableMouseCapture` to crossterm at startup; pressing `M` MUST toggle the bit at runtime, emitting `EnableMouseCapture` or `DisableMouseCapture` accordingly. The TUI MUST emit `DisableMouseCapture` on shutdown when the bit is on.

## Rationale
Crossterm's mouse capture interferes with terminal text selection and scrollback; users opt in explicitly.

## Derived from
- [[Mouse Capture is Opt-In]]
