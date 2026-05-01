---
type: HLR
tags:
  - req/hlr
  - domain/static
---
The dashboard single-page application is embedded in the binary at compile time so a single executable serves both the API and the UI without external static-file dependencies.

## Derived LLRs
- [[Static handler serves embedded asset by path]]
- [[Static handler SPA fallback to index html]]
- [[Static handler returns 404 when index missing]]
- [[Static handler sets content type from extension]]
