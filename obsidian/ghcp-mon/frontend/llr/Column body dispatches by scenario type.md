---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
`ColumnBody` MUST switch on `column.scenarioType` and render the matching scenario component: `live_sessions → LiveSessionsScenario`, `spans → SpansScenario`, `tool_detail → ToolDetailScenario`, `raw_browser → RawBrowserScenario`, `chat_detail → ChatDetailScenario`, `file_touches → FileTouchesScenario`.

## Rationale
The scenario dispatch is the only place that needs to know the full set of scenario types.

## Derived from
- [[Workspace Layout]]
