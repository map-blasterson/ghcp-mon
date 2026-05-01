---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
When `ToolDetailScenario` routes a span to `ExternalToolDetailBody` (i.e. `projection.tool_call` is absent and `projection.external_tool_call` is present), the rendered header section MUST contain an `<h4>` showing `ext.tool_name ?? "(unknown tool)"` followed by a kv block with these keys, in order:

- `call_id` — `ext.call_id ?? "—"`
- `tool_type` — the literal string `"external"` (the projection has no `tool_type` field)
- `duration` — `fmtNs(span.duration_ns ?? (span.end_unix_ns - span.start_unix_ns))`, falling back to `fmtNs(null)` when either bound is missing
- `start` — `fmtClock(span.start_unix_ns)`
- `conv` — the first 8 characters of `ext.conversation_id`, or `"—"` when missing
- `paired_tool_call_pk` — `ext.paired_tool_call_pk ?? "—"`
- `agent_run_pk` — `ext.agent_run_pk ?? "—"`

## Rationale
Mirrors the kv layout of `ToolDetailBody` so MCP / external-origin tool spans surface the same identifying metadata, while exposing the external-only fields (`paired_tool_call_pk`, `agent_run_pk`) that let the user correlate a paired native tool call.

## Derived from
- [[Tool Call Inspector]]
