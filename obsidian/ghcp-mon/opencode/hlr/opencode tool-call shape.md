---
type: HLR
tags:
  - req/hlr
  - domain/vendor-adapter
  - vendor/opencode
---
Tool spans whose `service_name === "opencode"` use the opencode SDK's tool catalog, argument names, and structured result envelopes (`{title, output, metadata}`). This HLR collects the normative mappings from opencode's raw tool-call data into the dashboard's normalized tool-call vocabulary (see the "Normalized tool-call vocabulary" sections of [[Tool Call Inspector]], [[Trace and Span Explorer]], and [[File Touch Tree]]), plus the opencode-only read-body envelope unwrap. The active adapter is selected per-span by `service_name`.

## Derived LLRs
- [[opencode tool-name mapping]]
- [[opencode tool-call argument mapping]]
- [[opencode tool-call result envelope]]
- [[opencode read body XML envelope unwrap]]
