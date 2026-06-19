---
type: impl
source: src/tui/widgets/confirm_modal.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Centered confirm modal with `[ y ]es` / `[ N ]o` buttons. Default selection is No per [[TUI Confirm modal default no]]. `y`/`Y` confirms; `n`/`N`/`Esc` cancels; `←`/`→`/`Tab` toggles selection; `Enter` dismisses with the current selection. Returns `Option<bool>` from `handle_key` so the caller can act on confirm.

## Source For
- [[TUI Confirm modal default no]]
- [[Delete session confirms and clears column session]]
