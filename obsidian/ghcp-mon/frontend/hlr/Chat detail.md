---
type: HLR
tags:
  - req/hlr
  - domain/input-breakdown
---
For a selected `chat` span, the dashboard renders the GenAI content attributes — system instructions, tool definitions, input messages, output messages — as an expandable byte-sized tree with a proportional summary bar so the user can see exactly what is being sent into the model.

## Derived LLRs
- [[Chat detail only renders for chat span]]
- [[Chat detail tree built from four content attributes]]
- [[Chat detail bytes computed via JSON length]]
- [[Chat detail summary bar proportional to visible segments]]
- [[Chat detail long primitives click to expand]]
- [[Chat detail mode toggle DELTA FULL]]
- [[Chat detail DELTA diffs against prior chat span]]
- [[Chat detail system instructions word-diff in DELTA]]
- [[Chat detail tool defs name-diff in DELTA]]
- [[Chat detail tool-call hint auto-expand and arrow]]
- [[Chat detail key cursor icon follows pointer]]
- [[Content attrs accepts object or json string]]
- [[Content parses input output messages]]
- [[Content parses tool call arguments]]
- [[Content parses tool call result]]
- [[Content has captured content predicate]]
- [[Content fmtNs adaptive units]]
- [[Content fmtClock and fmtRelative]]
- [[Content prettyJson safe fallback]]
- [[Message view renders parts by type]]
