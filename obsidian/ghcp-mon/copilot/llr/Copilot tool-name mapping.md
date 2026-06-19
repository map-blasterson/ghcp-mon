---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/copilot
---
When `service_name === "github-copilot"`, the Copilot tool-call adapter SHALL resolve raw tool names to normalized tool kinds as follows: `view` → `read`; `edit` → `edit`; `create` → `write`; `apply_patch` → `patch`; `bash` → `shell`. Tool names outside this set MUST remain unmapped (no normalized kind).

## Rationale
The Copilot CLI's tool catalog is fixed; this mapping is the single source of truth that UI scenarios consult when filtering or branching on normalized tool kind.

## Derived from
- [[Copilot tool-call shape]]
