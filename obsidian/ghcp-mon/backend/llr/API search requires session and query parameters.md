---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/search` MUST return `400 Bad Request` when `session` or `q` is absent or empty.

## Rationale
Search without a session would scan the entire database, defeating the purpose of session-scoped investigation. Requiring both parameters prevents accidental expensive queries.

## Derived from
- [[Dashboard REST API]]
