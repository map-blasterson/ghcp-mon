---
type: HLR
tags:
  - req/hlr
  - domain/input-breakdown
---
For a selected `chat` span, the dashboard renders the GenAI content attributes — system instructions, tool definitions, input messages, output messages — as an expandable byte-sized tree with a proportional summary bar so the user can see exactly what is being sent into the model.

## Derived LLRs
- [[Input breakdown only renders for chat span]]
- [[Input breakdown tree built from four content attributes]]
- [[Input breakdown bytes computed via JSON length]]
- [[Input breakdown summary bar proportional to visible segments]]
- [[Input breakdown long primitives click to expand]]
- [[Content attrs accepts object or json string]]
- [[Content parses input output messages]]
- [[Content parses tool call arguments]]
- [[Content parses tool call result]]
- [[Content has captured content predicate]]
- [[Content fmtNs adaptive units]]
- [[Content fmtClock and fmtRelative]]
- [[Content prettyJson safe fallback]]
- [[Message view renders parts by type]]
