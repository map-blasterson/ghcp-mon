---
type: impl
source: src/tui/api.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Async REST client (reqwest) mirroring web/src/api/client.ts; query-string helper that skips empty values and percent-encodes; throws on non-2xx.

## Source For
- [[API base URL hardcoded to local backend]]
- [[API client throws on non-2xx responses]]
- [[API client query string encoding]]
- [[API list methods apply default page size]]
- [[API client deleteSession uses DELETE]]
