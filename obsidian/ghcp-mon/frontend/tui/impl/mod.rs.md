---
type: impl
source: src/tui/mod.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
TUI subcommand entry point. Initializes the WS bus, REST client, panic-hook guard, and event sources; runs the draw loop; restores the terminal on exit.

## Source For
- [[Server URL Normalization and Reconnect Status]]
- [[TUI URL normalization rules]]
- [[TUI panic hook restores terminal]]
- [[TUI mouse capture opt-in via flag and runtime toggle]]
- [[TUI bus singleton lazy start]]
