---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
When `column.config.selected_tool_call_id` is set on a `chat_detail` column and the loaded chat span contains a `tool`-role input message that has a `tool_call_response` part with `id === selected_tool_call_id`, `ChatDetailScenario` MUST: (1) auto-add `root`, `root/input`, `root/input/input_messages`, and `root/input/input_messages/${i}` (the matching message's id path) to the `expanded` set without removing any user-opened nodes; and (2) render a `<div class="ib-target-arrow" aria-hidden>▶</div>` positioned at `top = elRect.top - wrapRect.top + wrap.scrollTop + elRect.height / 2` of the `[data-ib-id="…"]` row inside the scrollable tree wrap, recomputed whenever the target id, the rebuilt tree, or the expand state changes.

## Rationale
Auto-routes a user from an `execute_tool` selection straight to the corresponding tool message in chat detail without disturbing prior expand state.

## Derived from
- [[Chat detail]]
- [[Spans execute_tool selection auto-advances chat detail]]
