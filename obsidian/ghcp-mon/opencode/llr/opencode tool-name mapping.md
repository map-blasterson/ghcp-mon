---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/opencode
---
When `service_name === "opencode"`, the opencode tool-call adapter SHALL resolve raw tool names to normalized tool kinds as follows: `read` → `read`; `write` → `write`; `edit` → `edit`; `powershell` → `shell`. Tool names outside this set MUST remain unmapped (no normalized kind).

## Rationale
The opencode SDK's tool catalog is fixed; this mapping is the single source of truth that UI scenarios consult when filtering or branching on normalized tool kind.

## Derived from
- [[opencode tool-call shape]]
