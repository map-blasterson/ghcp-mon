---
type: impl
source: src/tui/app.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Top-level App: workspace, focus enum (None/Column(usize)/Widget), keymap+log overlay visibility, app-popover (AddColumn), confirm modal, per-column `Box<dyn Scenario>` map, cross-column `hovered_chat_pk` Arc<RwLock>, generation-keyed `span_detail_memo`. **No mouse handling.** Event loop is event-driven via `tokio::select!` over WS envelope broadcast, WS status broadcast, crossterm `EventStream`, cache `Notify`, plus an optional animation deadline (min of next reveal-queue head and next 250 ms boundary, gated on `DrawOutcome.spinner_visible`). After the first arm resolves, pending WS envelopes are drained via `try_recv` and applied in one `on_ws_envelopes(batch)` call that unions invalidation prefixes and runs at most one cache scan per dirtied prefix. Key dispatch walks five precedence layers (text-input > modal > widget > column > global). New columns inherit propagated `session`, `selected_*`, and `search_query` from the corresponding sibling propagation set so a column added after routing already ran doesn't open empty. Global keys: `q`/`Ctrl-C` quit (single press), `Tab`/`Shift-Tab` cycle focus, `Shift+←/→` move focused column, `Shift+Alt+←/→` resize focused column weight by ±0.1 clamped to [0.2, 5.0], `a` opens add-column popover, `x` removes focused column, `c` toggles Context Growth Widget, `Alt+↑/↓` (`+Shift` = 5 rows) resizes widget, `?` toggles keymap overlay, `~` toggles log overlay.

## Source For
- [[Terminal Event Loop]]
- [[TUI event loop is tokio select over five sources]]
- [[TUI drain pending events before draw]]
- [[TUI batch WS invalidation per loop cycle]]
- [[TUI animation deadline gates on spinner_visible]]
- [[TUI follow-mode advances on cache changed]]
- [[TUI top bar exposes WS status dot]]
- [[TUI top bar appends column via 'a' keystroke]]
- [[TUI top bar removes focused column via 'x' keystroke]]
- [[TUI top bar shows hint string for global keys]]
- [[TUI empty workspace renders hint text]]
- [[TUI terminal MIN_COL width 24 cells]]
- [[TUI columns below MIN collapse to ellipsis label]]
- [[TUI key-dispatch precedence text-input > modal > widget > column > global]]
- [[TUI Shift+Alt arrow column resize]]
- [[TUI single-press Ctrl-C quit]]
- [[TUI keymap overlay toggled by question mark]]
- [[TUI log overlay toggled by tilde]]
- [[TUI new column inherits propagated state]]
- [[TUI Context widget keyboard bar cursor navigation]]
- [[TUI Context widget height clamp in terminal rows]]
- [[TUI Context widget Alt arrow height adjustment]]
- [[TUI Context widget collapsed single-row bar]]
- [[TUI Context widget participates in Tab focus cycle]]
- [[TUI Scenario trait owns column behavior]]
