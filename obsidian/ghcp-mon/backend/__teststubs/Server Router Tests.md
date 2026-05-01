---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/server_router.rs`

Covers LLRs:
- [[OTLP router exposes traces metrics logs endpoints]] — `otlp_router_exposes_v1_traces`, `otlp_router_exposes_v1_metrics`, `otlp_router_exposes_v1_logs`.
- [[OTLP body limit 64 MiB]] — `otlp_router_permits_body_just_under_64_mib`, `otlp_router_rejects_body_over_64_mib`.
