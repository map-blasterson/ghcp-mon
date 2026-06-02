---
type: LLR
tags:
  - req/llr
  - tui
  - domain/workspace
---
When `App::append_column(scenario_type)` adds a new column to the workspace, the call MUST invoke `inherit_propagated_state(new_idx, &mut columns)` BEFORE focusing the new column and persisting. The inherit step MUST, using a first-non-empty lookup across all other columns, copy: `session` into Spans / ChatDetail / FileTouches columns; each of `selected_trace_id`, `selected_span_id`, `selected_tool_call_id` into Spans / ToolDetail / ChatDetail columns; and `search_query` into ChatDetail / ToolDetail columns. LiveSessions and RawBrowser columns MUST inherit nothing. Values MUST NOT overwrite existing values on the new column.

## Rationale
Regression bug "FileTouches opens empty until session is re-selected": columns added after session propagation already ran otherwise miss the broadcast value entirely.

## Derived from
- [[Keybinding Matrix]]
- [[Selecting session propagates to dependent columns]]
