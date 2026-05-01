---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
For tools without a specialized renderer, `ToolDetailScenario` MUST split the parsed arguments object into "code-ish" string fields (any string containing a newline) and the remainder; code-ish fields MUST be rendered as `<pre class="edit-diff">`, the remainder MUST be rendered as a single pretty-printed JSON block, and a string `result` MUST be rendered as `<pre>` while a non-string result MUST be rendered as pretty-printed JSON.

## Rationale
Generic tools have heterogeneous schemas; isolating multi-line strings keeps inline blobs readable while keeping structured fields scannable.

## Derived from
- [[Tool Call Inspector]]
