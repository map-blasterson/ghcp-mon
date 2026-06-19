---
type: LLR
tags:
  - req/llr
  - tui
  - domain/logging
---
When the subcommand is `Attach`, `main` MUST install a `tracing-appender::rolling::never` writer at `dirs::cache_dir()/ghcp-mon/tui.log` (creating the parent directory if missing) as a `tracing-subscriber` `fmt` layer. The layer MUST have ANSI escapes disabled so the log file is plain-text grepable.

## Rationale
Stderr writes corrupt the alternate-screen terminal; a rolling file is the canonical TUI log sink.

## Derived from
- [[TUI Logging via Rolling File and In-Process Overlay]]
