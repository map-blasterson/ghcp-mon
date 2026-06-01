---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/logging
---
While the TUI is attached, stderr writes corrupt the alternate-screen + raw-mode terminal. So:

1. `tracing-subscriber` is initialized inside each subcommand branch — not globally before dispatch.
2. For the `Attach` subcommand, install a `tracing-appender` rolling-file layer at `dirs::cache_dir()/ghcp-mon/tui.log` **and** an in-process buffer layer that retains the most recent ~500 records.
3. The `?` key toggles a modal overlay showing the in-process buffer.

For every non-`Attach` subcommand, the historical stderr fmt layer remains in place.

## Derived LLRs
- [[TUI rolling-file tracing layer]]
- [[TUI in-process log overlay buffer]]
- [[TUI logging routed to file not stderr while attached]]
