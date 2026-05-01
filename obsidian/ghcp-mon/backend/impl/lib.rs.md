---
type: impl
source: src/lib.rs
lang: rust
tags:
  - impl/original
  - impl/rust
---
Original source file for reverse-engineered requirements.

`lib.rs` declares the crate's public module tree (`api`, `db`, `error`, `ingest`, `local_session`, `model`, `normalize`, `server`, `static_assets`, `ws`) so the binary and integration tests share a single library boundary.

## Source For
- (no LLRs — pure module declarations)
