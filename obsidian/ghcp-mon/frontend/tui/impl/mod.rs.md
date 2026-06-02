---
type: impl
source: src/tui/mod.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
TUI subcommand entry point. Initializes the WS bus, REST client, panic-hook guard, and event sources; runs the draw loop via `app::event_loop`; restores the terminal on exit; persists workspace on shutdown. **No mouse capture** — `ratatui::try_init()` runs without `EnableMouseCapture` and the `--mouse` CLI flag and `M`-toggle are gone.

## Source For
- [[Server URL Normalization and Reconnect Status]]
- [[TUI URL normalization rules]]
- [[TUI panic hook restores terminal]]
- [[TUI bus singleton lazy start]]
