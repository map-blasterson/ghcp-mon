---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/replay_endpoint.rs`

Covers LLR:
- [[Replay endpoint accepts path and returns count]] — `replay_endpoint_returns_path_and_count` (POSTs `{"path":...}`, asserts `{"path", "ingested": 2}` response with mixed-content fixture, checks DB row count via `source='replay'`).
