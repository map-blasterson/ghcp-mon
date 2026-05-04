---
type: LLR
tags:
  - req/llr
  - domain/traces
---
`SpansScenario` MUST render a text input in the column config bar (alongside the existing session and kind selectors) and, when both a session is selected and the user has entered text, MUST debounce (300 ms) and call `GET /api/search?q=<text>&session=<cid>`. Clearing the searchbox or deselecting the session MUST restore the normal span tree view.

## Rationale
Server-side search lets the user locate spans by tool name, attribute, or event content. Debouncing avoids flooding the server with intermediate keystrokes. Clearing the query returns the column to its default tree-browse mode.

## Derived from
- [[Trace and Span Explorer]]
