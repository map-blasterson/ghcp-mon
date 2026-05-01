---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
The `JsonView` component MUST `JSON.stringify(value, null, 2)` and render the result inside `<pre class="json">`; when `collapsed` is true it MUST wrap the `<pre>` in a `<details>` element with summary `"json…"`. `JSON.stringify` failures MUST fall back to `String(value)`.

## Rationale
Consistent JSON rendering across inspectors; cycles fall back gracefully instead of throwing.

## Derived from
- [[Tool Call Inspector]]
