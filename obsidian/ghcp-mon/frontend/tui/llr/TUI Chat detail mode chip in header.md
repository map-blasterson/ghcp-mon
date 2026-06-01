---
type: LLR
tags:
  - req/llr
  - tui
  - domain/input-breakdown
---
The Chat Detail column header MUST render a `mode: [DELTA]` or `mode: [FULL]` chip at fixed horizontal position in the 1-row header strip, styled `Color::Cyan` BOLD. The `m` keystroke MUST toggle the chip between the two values atomically (`DELTA` → `FULL` → `DELTA` …), persisting the chosen value in `column.config.chat_mode` so sibling columns reading the same key see the toggle. The chip MUST be visible regardless of selection / loading state — the chip is part of the column chrome, not the body.

## Rationale
Per the shared [[Chat detail mode toggle DELTA FULL]] LLR the user must be able to opt out of diff mode without leaving the column; the chip in the header is the terminal analog of the web's button group.

## Derived from
- [[Chat detail mode toggle DELTA FULL]]
