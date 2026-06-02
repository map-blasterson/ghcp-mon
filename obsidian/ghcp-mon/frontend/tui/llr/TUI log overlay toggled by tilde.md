---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
At the global key layer, `~` MUST toggle `App::log_overlay_visible`. While `log_overlay_visible` is true, the modal layer MUST consume every key — only `~` or `Esc` clears the flag, and all other keys are swallowed.

## Rationale
The log overlay is split from the keymap overlay so each gets its own dedicated binding.

## Derived from
- [[Keybinding Matrix]]
- [[TUI Logging via Rolling File and In-Process Overlay]]
