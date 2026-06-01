---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
Pressing `a` at the global precedence level MUST append a new column to the workspace using the next `ScenarioType` in the round-robin (`live_sessions → spans → tool_detail → chat_detail → file_touches → raw_browser`), MUST persist the workspace after the change, and MUST focus the appended column when no column was previously focused.

## Rationale
Quick way to add a column without a popup menu — Phase 0 does not yet have a scenario picker.

## Derived from
- [[Top Bar and Status Dot]]
- [[Keybinding Matrix]]
