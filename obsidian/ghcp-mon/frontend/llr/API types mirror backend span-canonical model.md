---
type: LLR
tags:
  - req/llr
  - domain/api-client
---
`web/src/api/types.ts` MUST declare the TypeScript types the API client returns (`SessionSummary`, `SessionDetail`, `SpanRow`, `SpanFull`, `SpanDetail`, `SpanNode`, `SessionSpanTreeResponse`, `TraceSummary`, `TraceDetailResponse`, `ContextSnapshot`, `RawRecord`, the WS envelope/payload shapes, and the `KindClass` and `RawRecordType` string-literal unions) such that they structurally match the JSON emitted by the backend's `src/api/mod.rs` and `src/ws/*` modules.

## Rationale
Component code is fully typed against these declarations; if the backend changes a field the TypeScript build catches the drift.

## Derived from
- [[REST API Client]]
- [[API router exposes session and span endpoints]]
