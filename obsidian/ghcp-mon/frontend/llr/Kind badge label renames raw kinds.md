---
type: LLR
tags:
  - req/llr
  - domain/traces
---
The `kindLabel` function MUST map `KindClass` values to display labels as follows: `execute_tool → "tool"`, `external_tool → "external"`, `invoke_agent → "agent"`, `other → "pending"`, and any other value to itself unchanged.

## Rationale
Display strings tuned for readability; the wire/DB representation stays untouched.

## Derived from
- [[Trace and Span Explorer]]
