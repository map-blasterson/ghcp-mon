---
type: HLR
tags:
  - req/hlr
  - domain/db
---
The system persists raw ingested payloads and normalized telemetry to a local SQLite database, ensuring durability, idempotent reconcile across re-deliveries, and traceability from derived rows back to their source raw records.

## Derived LLRs
- [[DB open creates parent directory]]
- [[DB open enables WAL and foreign keys]]
- [[DB open runs migrations]]
- [[DAO insert_raw returns row id]]
- [[Raw request body persisted verbatim per OTLP request]]
- [[Each envelope persisted as own raw record]]
