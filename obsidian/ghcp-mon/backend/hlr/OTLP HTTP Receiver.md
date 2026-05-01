---
type: HLR
tags:
  - req/hlr
  - domain/otlp
---
The system accepts OpenTelemetry telemetry over OTLP/HTTP (JSON encoding) on a dedicated listener, persists each request body verbatim, and converts each request into the internal envelope format for normalization.

## Derived LLRs
- [[OTLP router exposes traces metrics logs endpoints]]
- [[OTLP rejects protobuf content type]]
- [[OTLP traces persists raw and normalizes envelopes]]
- [[OTLP metrics persists raw and normalizes envelopes]]
- [[OTLP logs persisted raw only]]
- [[OTLP body limit 64 MiB]]
- [[OTLP attribute flattening]]
- [[OTLP int value parsed as int64]]
