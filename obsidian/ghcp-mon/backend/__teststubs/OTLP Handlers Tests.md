---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/otlp_handlers.rs`

Covers LLRs:
- [[OTLP rejects protobuf content type]] — `protobuf_content_type_returns_501_on_traces`, `protobuf_content_type_returns_501_on_metrics_and_logs` (latter also exercises case-insensitivity and `; charset=...` suffix).
- [[OTLP traces persists raw and normalizes envelopes]] — `traces_persists_raw_and_normalizes_envelopes`.
- [[OTLP metrics persists raw and normalizes envelopes]] — `metrics_persists_raw_and_normalizes_envelopes`.
- [[OTLP logs persisted raw only]] — `logs_persisted_raw_only_no_normalized_rows`.
