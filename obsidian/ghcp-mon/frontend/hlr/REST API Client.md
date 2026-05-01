---
type: HLR
tags:
  - req/hlr
  - domain/api-client
---
The dashboard reaches the backend through a single typed API client that issues HTTP requests against a fixed base URL and returns shape-preserving TypeScript values mirroring the server's span-canonical model.

## Derived LLRs
- [[API base URL hardcoded to local backend]]
- [[API client throws on non-2xx responses]]
- [[API client query string encoding]]
- [[API list methods apply default page size]]
- [[API client deleteSession uses DELETE]]
- [[API types mirror backend span-canonical model]]
