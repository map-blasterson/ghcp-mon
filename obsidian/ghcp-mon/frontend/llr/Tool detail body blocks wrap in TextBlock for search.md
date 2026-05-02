---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
Every body field rendered by the tool-detail arg renderers (`GenericArgs`, `EditArgs`, `ViewArgs`, `TaskArgs`, `ReadAgentArgs`) — arguments JSON, code-ish args, `old_str`/`new_str` code blocks, `view`-tool result body, `task`/`read_agent` markdown bodies, and string/JSON results — MUST be wrapped in a `<TextBlock searchable>` element so the per-block search affordance is available everywhere a long blob is rendered. Pure key/value chip rows (the `.kv` blocks) MAY remain outside `TextBlock`.

## Rationale
Every long blob in tool detail benefits from in-place search; key/value chip rows are too short to bother.

## Derived from
- [[Tool Call Inspector]]
- [[Searchable Text Block]]
