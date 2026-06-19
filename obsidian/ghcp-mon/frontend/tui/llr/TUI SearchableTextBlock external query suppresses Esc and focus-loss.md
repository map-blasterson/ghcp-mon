---
type: LLR
tags:
  - req/llr
  - tui
  - domain/text-block
---
When `SearchableTextBlock`'s `external_query` prop is `Some(non-empty)`, the widget's interactive exit gestures MUST be suppressed: `handle_key` MUST NOT exit the `Active` phase on `Esc` (the key is treated as a no-op, not consumed, so the host may route it elsewhere), and `on_focus_lost` MUST be a no-op. The widget's `query` is replaced verbatim by the external value and the embedded input shows it read-only (edit keys are ignored while externally driven). The `Active` phase ends only when the prop transitions to `None` or `Some("")`, detected in `render` via the private `external_active` latch; on that transition the widget clears `query`/`match_index`/`match_count`/`scroll_top` and returns to `Icon` (if focused) or `Idle`. Entering `Active` from `Idle`/`Icon` when the prop becomes `Some(non-empty)` requires no keypress.

## Rationale
Externally-driven search (e.g. the Spans search box propagated into Tool/Chat detail) must own the lifecycle exclusively; an accidental `Esc` or focus change inside the highlighted block must not cancel a search the user did not start interactively. The `external_active` latch lets `render` detect the prop's falling edge without the caller threading the previous value.

## Derived from
- [[TextBlock external query suppresses interactive exit gestures]]
- [[TextBlock accepts external search query prop]]
