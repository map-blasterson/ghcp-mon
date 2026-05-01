---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
On first run (no persisted state) and after `resetDefault()`, the workspace MUST contain exactly four columns in order: `live_sessions` (width 1, title "Sessions"), `spans` (width 1.4, title "Traces"), `tool_detail` (width 1.4, title "Tool detail"), `input_breakdown` (width 1.6, title "Input breakdown"). `resetDefault` MUST also set `contextWidgetHeightVh` to `15` and `contextWidgetVisible` to `true`.

## Rationale
Defines the out-of-the-box dashboard layout.

## Derived from
- [[Workspace Layout]]
