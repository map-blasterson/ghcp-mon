---
type: LLR
tags:
  - req/llr
  - tui
  - domain/workspace
---
On first run (no persisted state) and on `Workspace::reset_default`, the workspace MUST contain exactly four columns in order: `live_sessions` (width 1.0, title "Sessions"), `spans` (width 1.4, title "Spans"), `tool_detail` (width 1.4, title "Tool detail"), `chat_detail` (width 1.6, title "Chat detail"). `reset_default` MUST also set `context_widget_height_rows` to 15 and `context_widget_visible` to `true`.

## Rationale
Defines the out-of-the-box TUI layout, matching the web defaults.

## Derived from
- [[Workspace Persistence]]
- [[Default workspace seeds four columns]]
