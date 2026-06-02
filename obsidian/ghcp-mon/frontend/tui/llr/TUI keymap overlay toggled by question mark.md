---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
At the global key layer, `?` MUST toggle `App::keymap_overlay_visible`. While `keymap_overlay_visible` is true, the modal layer MUST consume every key — only `?` or `Esc` clears the flag, and all other keys are swallowed (no fall-through). The overlay's body MUST be assembled from `App::active_keymap_entries`, which returns the scenario's text-input keymap exclusively when the focused scenario is in text-input mode, otherwise the global keymap plus either the context-widget keymap (when widget is focused) or the focused column's `Scenario::keymap_entries`.

## Rationale
Replaces the old `?`-toggles-log binding so the help overlay is reachable with the conventional `?` key while logs get their own toggle.

## Derived from
- [[Keybinding Matrix]]
