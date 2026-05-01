---
type: impl
source: src/local_session.rs
lang: rust
tags:
  - impl/original
  - impl/rust
---
Original source file for reverse-engineered requirements.

Reads the Copilot CLI's per-conversation `workspace.yaml` sidecar to recover human-readable session metadata that OTel does not carry.

## Source For
- [[Local session state dir resolved from flag env or home]]
- [[Local session workspace yaml rejects path traversal]]
- [[Local session workspace yaml best effort read]]
