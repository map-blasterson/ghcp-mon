---
type: LLR
tags:
  - req/llr
  - domain/api-client
---
`api.deleteSession(cid)` MUST issue an HTTP `DELETE` to `/api/sessions/{cid}` and resolve to the parsed JSON body `{ deleted: boolean; conversation_id: string; trace_count: number }`.

## Rationale
Deleting a conversation cascades through its traces and projections on the backend; the response surfaces the trace count for UI confirmation.

## Derived from
- [[REST API Client]]
- [[API delete session purges traces and projections]]
