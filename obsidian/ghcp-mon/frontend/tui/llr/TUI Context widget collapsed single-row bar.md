---
type: LLR
tags:
  - req/llr
  - tui
  - domain/context-growth
---
The global key `c` MUST toggle Context Growth Widget visibility (`context_widget_visible`), guarded so it does not collide with `Ctrl-C` (quit) or `Alt+c`. When hidden, the widget occupies a single bottom row rendered as a collapsed affordance (`▾ context growth  (press c to expand)`) and is dropped from the `Tab` focus cycle; widget focus is cleared. Toggling persists the workspace TOML. This realizes `Context widget hide and show toggle` in the terminal.

## Rationale
A single-row collapsed bar keeps the toggle discoverable while reclaiming vertical space for the workspace.

## Derived from
- [[Context widget hide and show toggle]]
- [[Keybinding Matrix]]
