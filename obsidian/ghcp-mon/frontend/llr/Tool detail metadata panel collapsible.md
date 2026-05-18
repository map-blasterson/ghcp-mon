---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
The header of `ToolDetailBody` and `ExternalToolDetailBody` MUST be a native HTML `<details className="section">` element whose `<summary>` contains the tool-name heading (`<h4>{tool_name ?? "(unknown tool)"}</h4>`) and whose body is the key/value metadata grid (`call_id`, `tool_type`, `duration`, `status`/`paired_tool_call_pk`, `start`, `conv`, etc.). The `<details>` element MUST default to closed (no `open` attribute).

## Rationale
The metadata panel is a quick-reference block that the user normally already knows in context; collapsing it by default keeps the args/result and hero panels above the fold while still exposing the metadata one click away.

## Derived from
- [[Tool Call Inspector]]
