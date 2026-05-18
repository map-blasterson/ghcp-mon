---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For native tool spans whose `projection.tool_call.tool_type === "function"`, `ToolDetailBody` MUST surface the first present argument from the priority list `["command", "query", "description"]` into a dedicated hero panel rendered between the collapsible metadata panel and the `args / result` section. The hero panel MUST be a `<div className="section tool-hero">` containing a single `<pre className="tool-hero-value">{value}</pre>` — no key label, no border chrome on the value — where `value` is the argument value rendered as a string (string values verbatim, non-string values via `prettyJson`). The hero MUST be suppressed when: the arguments object is null; none of the priority keys are present; the value is null; or the rendered string is empty. Only the first matching priority key is surfaced (e.g., when both `command` and `description` are present, the hero MUST show `command`). The hero MUST NOT render for external (`external_tool_call`) tool spans nor for non-`function` tool_types.

## Rationale
For function tools, one argument is almost always the actionable parameter (the shell command, the search query, the agent description) and is what the user came to read. Hoisting it into a high-contrast hero panel removes the friction of opening the args section, while limiting the hero to `tool_type === "function"` keeps it out of edge-cases (e.g., `mcp`/`builtin` tool types where the canonical arg differs).

## Derived from
- [[Tool Call Inspector]]
