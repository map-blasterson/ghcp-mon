---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
The TUI confirm modal MUST default to the `No` selection on open. The
button row renders both choices `[ y ]es` and `[ N ]o`; the currently
selected button is shown with a yellow background + bold black foreground.

Keys:
- `y` / `Y` → confirm immediately, regardless of selection.
- `n` / `N` / `Esc` → cancel immediately, regardless of selection.
- `←` / `→` / `Tab` → toggle selection (Yes ↔ No) without dismissing.
- `Enter` → dismiss with the currently selected option (default: No).

## Rationale
Defaulting to No prevents accidental destructive actions when the modal
spawns under a held keyboard input. Explicit `y`/`n` shortcuts let
confident users skip the toggle.

## Derived from
- [[Key-Dispatch Policy]]
- [[Delete session confirms and clears column session]]
