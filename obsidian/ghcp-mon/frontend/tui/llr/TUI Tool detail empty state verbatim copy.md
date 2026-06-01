---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
The tool-detail column MUST render these empty/placeholder states verbatim: with no span selection → `Select a tool span in the Spans column.`; with a selection whose detail is still being fetched → `loading…`; with a resolved span that has neither a `tool_call` nor `external_tool_call` projection → `selected span is not a tool call`; and, inside a renderer, when a tool span captured no args and no result → `no content captured — set OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true and OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental`. The no-content copy MUST match the web `NO_CONTENT_LINE` string exactly.

## Rationale
Identical copy keeps the terminal and web UIs in lockstep and gives users the exact environment variables to enable content capture.

## Derived from
- [[Tool detail empty state when no content captured]]
- [[Tool detail requires tool call projection]]
