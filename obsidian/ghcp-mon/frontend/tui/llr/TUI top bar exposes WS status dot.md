---
type: LLR
tags:
  - req/llr
  - tui
  - domain/workspace
---
The first cell of the TUI's one-row top bar MUST render a `StatusDot` whose foreground colour reflects `WsBus::status()`: green for `Connected`, yellow for `Connecting`/`Reconnecting`, red for `Error`. The dot's title text MUST be `"connected"`, `"connecting"`, `"reconnecting"`, or `"ws error"` accordingly, and MUST be rendered as part of the top bar's hint string.

## Rationale
Always-visible indicator of live-feed health, matching the web top bar's dot.

## Derived from
- [[Top Bar and Status Dot]]
- [[Top bar exposes ws connection indicator]]
