---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/tool-detail
---
The Tool Detail column renders an inline unified line-level diff for `edit`-tool tool calls (the `old_str` → `new_str` change), with `+`/`-` markers, red/green per-row coloring, and per-line gutter line numbers. The diff participates in per-block `/` search and external-query highlighting like any other body block. The `+/-` chip on the Spans row for the same edit reflects the actual number of changed diff rows, not the totals of `old_str` / `new_str` lines.

## Derived LLRs
- [[TUI Tool detail edit inline diff renders unified rows]]
- [[TUI Tool detail edit diff chip counts diff rows not block totals]]
