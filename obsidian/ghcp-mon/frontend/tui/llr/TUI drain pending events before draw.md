---
type: LLR
tags:
  - req/llr
  - tui
  - domain/event-loop
---
The TUI event loop MUST drain every pending `AppEvent` via `mpsc::Receiver::try_recv` after handling the first event of a cycle, and MUST call `terminal.draw` exactly once per cycle.

## Rationale
One frame per draw, not one draw per event — guarantees the event queue never grows faster than it drains.

## Derived from
- [[Terminal Event Loop]]
