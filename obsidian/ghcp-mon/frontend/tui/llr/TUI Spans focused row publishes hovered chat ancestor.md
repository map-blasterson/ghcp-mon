---
type: LLR
tags:
  - req/llr
  - tui
  - domain/traces
---
The keyboard analog of the mouse-hover variant in
[[Span tree row publishes hovered chat ancestor]]: on every focused-row
change in the Spans column (`↑`/`↓`/`Home`/`End`/`PageUp`/`PageDown`), the
App MUST locate the focused row's nearest chat ancestor in the loaded
session span tree (or use the row itself when its `kind_class` is `chat`)
and write its `span_pk` (or `None` when no chat is in the ancestor path)
to a single shared `Arc<RwLock<Option<i64>>>` held on `App` state. Phase 2
(Context Growth Widget) consumes that store to highlight the matching bar.

When the column loses focus (e.g., `Tab` to another column), the App MUST
publish `None`.

## Rationale
A single hover store lets every column observe selection without per-pair
wiring. Mouse capture is opt-in; the keyboard path is always present.

## Derived from
- [[Terminal Rendering Constraints]]
