---
type: LLR
tags:
  - req/llr
  - tui
  - domain/text-block
---
The web `TextBlock` renders a decorative `?/` glyph that follows the mouse cursor with a 12px lead on every `mousemove`. A terminal has no continuous pointer-coordinate stream and no sub-cell positioning, so the TUI port substitutes a **static `[?]` hint** painted in the top-right corner of the block (`area.x + width - 3`, top row) while the block is in the `Icon` phase (focused but search not yet active). The hint is a single `Span` styled `Color::DarkGray` + `Modifier::DIM`. Idle→Icon is driven by `focused` becoming true in `render`; Icon→Idle when `focused` becomes false. The hint is never painted in the `Idle` or `Active` phases.

## Rationale
The cursor-following glyph is a mouse-affordance discovery cue; in a keyboard-first TUI the equivalent affordance is a fixed, low-contrast corner marker that signals "press `/` to search" without intercepting any cells the body needs.

## Derived from
- [[TextBlock cursor-following search hint icon]]
- [[Terminal Rendering Constraints]]
