---
type: HLR
tags:
  - req/hlr
  - domain/api
---
The system exposes a JSON REST API over an HTTP listener so the dashboard frontend (and external tools) can list and inspect sessions, traces, spans, projections, and raw records, with permissive CORS for local-first usage.

## Derived LLRs
- [[API router exposes session and span endpoints]]
- [[API allows any origin via CORS]]
- [[API healthz endpoint]]
- [[API list sessions ordered by recency]]
- [[API session detail returns span count]]
- [[API session span tree trace scoped union]]
- [[API list traces aggregates per trace]]
- [[API list traces floats placeholder only traces]]
- [[API get trace returns span tree]]
- [[API list spans filterable by session and kind]]
- [[API get span returns events parent children projection]]
- [[API list session contexts ordered by capture]]
- [[API list raw filterable by record type]]
- [[API list query limit clamped]]
