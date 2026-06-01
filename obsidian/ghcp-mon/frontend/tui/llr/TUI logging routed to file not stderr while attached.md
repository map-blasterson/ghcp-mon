---
type: LLR
tags:
  - req/llr
  - tui
  - domain/logging
---
When the subcommand is `Attach`, the `tracing-subscriber` initialization MUST NOT install any stderr `fmt` layer. Every `tracing::*` event in the `Attach` lifetime MUST instead flow to (a) the rolling-file layer at `dirs::cache_dir()/ghcp-mon/tui.log` and (b) the in-process `LogBufferLayer`.

## Rationale
Stderr writes corrupt the alternate-screen terminal; this rule is the necessary plumbing change in `main.rs`.

## Derived from
- [[TUI Logging via Rolling File and In-Process Overlay]]
