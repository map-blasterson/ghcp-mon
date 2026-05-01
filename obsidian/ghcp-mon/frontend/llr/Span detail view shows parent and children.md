---
type: LLR
tags:
  - req/llr
  - domain/traces
---
The `SpanDetailView` `relations` section MUST display the parent as `"<parent.name> (<8-char span_id>)"` (or `—` when there is no parent) and MUST list every child `SpanRef` with its name, `KindBadge`, and 8-character span id.

## Rationale
Lets the user navigate sibling/parent context without leaving the inspector.

## Derived from
- [[Trace and Span Explorer]]
