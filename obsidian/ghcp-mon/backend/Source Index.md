# Backend Source Index

`ghcp-mon` — local-first GitHub Copilot CLI telemetry collector and dashboard backend, written in Rust. Receives OTLP/HTTP and file-exporter JSON-lines telemetry, normalizes spans into a canonical relational model with idempotent reconcile, exposes a REST API and live WebSocket event stream, and embeds a Vite SPA for the dashboard UI.

See [[Source Files]] for the manifest of files reverse-engineered into this vault.

## High-Level Requirements
- [[CLI Entry Point]]
- [[OTLP HTTP Receiver]]
- [[File Exporter Replay]]
- [[Telemetry Persistence]]
- [[Span Normalization]]
- [[Dashboard REST API]]
- [[Live WebSocket Event Stream]]
- [[Embedded Dashboard SPA]]
- [[Uniform Error Reporting]]
- [[Local Session Metadata]]
