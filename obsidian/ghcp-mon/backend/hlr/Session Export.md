---
type: HLR
tags:
  - req/hlr
  - domain/export
---
The system can export an existing session's spans as a replay-compatible JSON-lines stream so that operators can archive sessions, share reproductions, or feed them back through `ghcp-mon replay` to reconstitute the session in a fresh database. The export is span-only — metrics and logs are intentionally excluded as the dashboard treats spans as the canonical projection.

## Derived LLRs
- [[Export session exists preflight]]
- [[Export streams spans as JSON lines]]
- [[Export missing session returns NotFound]]
- [[Export trace scoped union of session spans]]
- [[Export includes span events]]
- [[Export ordered by start time]]
- [[Export excludes metrics and logs]]
- [[Export warns on NULL start unix ns]]
- [[CLI export subcommand emits session JSON lines]]
- [[CLI export missing session exits non-zero]]
- [[CLI tracing diagnostics routed to stderr]]
