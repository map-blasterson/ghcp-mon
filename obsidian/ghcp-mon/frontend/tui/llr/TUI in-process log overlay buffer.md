---
type: LLR
tags:
  - req/llr
  - tui
  - domain/logging
---
When the subcommand is `Attach`, a custom tracing layer (`LogBufferLayer`) MUST capture every emitted event into a thread-safe ring buffer (`LogBuffer`) capped at `LOG_BUFFER_CAP = 500` records. The buffer's snapshot MUST be displayed in a modal overlay when the `?` key is pressed.

## Rationale
Gives the user immediate diagnostic access without leaving the TUI.

## Derived from
- [[TUI Logging via Rolling File and In-Process Overlay]]
