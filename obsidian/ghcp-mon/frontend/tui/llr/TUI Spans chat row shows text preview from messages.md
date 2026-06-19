---
type: LLR
tags:
  - req/llr
  - tui
  - domain/spans
---
For Chat-kind spans whose name is suppressed by display_name, the Spans row description slot MUST come from `chips::chat_text_preview(attrs)`, which MUST prefer the last `Part::Text` content found in `gen_ai.output.messages` (assistant reply); when no output text exists it MUST fall back to the LAST `Part::Text` in the REVERSED `gen_ai.input.messages` (freshest user prompt, not the stale system primer at index 0). The returned string MUST collapse all internal whitespace runs to single spaces (so the one-row layout is never blown by embedded newlines) and MUST NOT be length-capped — width-truncation with `…` is the renderer's responsibility. When no previewable text exists (tool-call-only chats, empty attrs), the function MUST return `None`.

## Rationale
Replaces the redundant "chat" row label with the actual conversation turn the user wants to scan.

## Derived from
- [[TUI Spans tree row layout in cells]]
