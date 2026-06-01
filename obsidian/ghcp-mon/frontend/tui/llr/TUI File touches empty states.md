---
type: LLR
tags:
  - req/llr
  - tui
  - domain/file-touches
---
The File Touches column MUST render exactly these verbatim empty-state strings, in precedence order: when no `session` is configured → `pick a session`; when a session is configured but the `["session-span-tree", session]` cache entry has no value yet → `loading…`; when the tree is loaded but no file-touching tool spans have been extracted → `no file touches yet`. All three MUST render in DarkGray + ITALIC.

## Rationale
Distinguishing "loading" from "no touches" requires inspecting whether the session-span-tree cache key holds a value (peeked by the column's draw method), not merely whether the extracted touch list is empty — an empty list is ambiguous between the two.

## Derived from
- [[File Touch Tree]]
- [[File touches aggregates view edit create]]
