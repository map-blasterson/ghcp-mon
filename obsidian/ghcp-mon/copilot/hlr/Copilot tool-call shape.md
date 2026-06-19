---
type: HLR
tags:
  - req/hlr
  - domain/vendor-adapter
  - vendor/copilot
---
Tool spans whose `service_name === "github-copilot"` use the Copilot CLI's tool catalog and argument-naming conventions. This HLR collects the normative mappings from Copilot's raw tool-call data into the dashboard's normalized tool-call vocabulary (see the "Normalized tool-call vocabulary" sections of [[Tool Call Inspector]], [[Trace and Span Explorer]], and [[File Touch Tree]]), plus the Copilot-only `apply_patch` parsing needed for the file-touch tree and diff-stat badge. The active adapter is selected per-span by `service_name`.

## Derived LLRs
- [[Copilot tool-name mapping]]
- [[Copilot tool-call argument mapping]]
- [[Copilot tool-call result envelope]]
- [[Copilot apply_patch path extraction]]
- [[Copilot apply_patch diff-stat parsing]]
