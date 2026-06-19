---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
The tool-detail metadata panel (native and external bodies) MUST default to collapsed. When closed it renders only a summary row — the tool name prefixed with `▸` — and MUST NOT render the key/value grid. `Space` on the focused metadata panel toggles it open, switching the glyph to `▾` and revealing the kv rows. Native kv fields: `call_id`, `tool_type`, `duration`, `status`, `start`, `conv` (first 8 of conversation id). External kv fields: `call_id`, `tool_type` = `external`, `duration`, `start`, `conv` (first 8), `paired_tool_call_pk`, `agent_run_pk`. Missing values render as `—`.

## Rationale
Mirrors the web `<details>` metadata section (default closed) so the column opens focused on args/result; the disclosure triangle and field set match the web header layout.

## Derived from
- [[Tool detail metadata panel collapsible]]
- [[External tool detail body header fields]]
