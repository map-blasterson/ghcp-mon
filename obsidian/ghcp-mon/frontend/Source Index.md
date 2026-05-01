# Frontend Source Index

`ghcp-mon` web dashboard — a single-page React 18 + Vite + TanStack Query application that monitors GitHub Copilot CLI's OpenTelemetry data via the backend API. Renders sessions, trace span trees, tool-call detail, chat-chat details, file-touch trees, raw OTel records, and a context-growth chart in a workspace of resizable columns. Live updates via a WebSocket subscription invalidate the relevant TanStack Query caches.

See [[Source Files]] for the manifest of files reverse-engineered into this vault.

## High-Level Requirements
- [[REST API Client]]
- [[Live WebSocket Subscription]]
- [[Workspace Layout]]
- [[Live Session Browser]]
- [[Trace and Span Explorer]]
- [[Tool Call Inspector]]
- [[Chat detail]]
- [[File Touch Tree]]
- [[Context Growth Widget]]
- [[Raw Record Browser]]
