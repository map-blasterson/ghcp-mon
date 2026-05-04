---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
While viewing a `chat`-kind span, `ChatDetailScenario` MUST render a `<span class="ib-mode-toggle" role="group" aria-label="view mode">` in the column header containing two `<button class="ib-mode-btn">` elements labeled `DELTA` and `FULL`; clicking either MUST set `column.config.chat_mode` to that value via `updateColumn`, and the active button MUST carry the additional class `active`. `chat_mode` defaults to `"DELTA"` when unset, and is persisted in the column config so sibling columns can read it.

## Rationale
The user must be able to opt out of diff mode (e.g., to read full system instructions) without leaving the column.

## Derived from
- [[Chat detail]]
