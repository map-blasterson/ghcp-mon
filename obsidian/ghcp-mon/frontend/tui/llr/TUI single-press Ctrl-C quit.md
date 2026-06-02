---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
A single `Ctrl-C` keypress at the global key layer MUST cause `App::handle_key` to return `Ok(true)`, terminating `event_loop`. No confirm-chord or repeat detection is required. Plain `q` (without `Shift`) MUST quit identically.

## Rationale
Standard terminal-app shortcut; matches users' habits coming from ratatui sample apps.

## Derived from
- [[Keybinding Matrix]]
