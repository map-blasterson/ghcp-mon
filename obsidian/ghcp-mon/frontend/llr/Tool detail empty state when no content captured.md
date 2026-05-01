---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
When a tool span has neither parsed arguments nor a parsed result, the `ToolDetailScenario` body MUST render `NO_CONTENT_LINE` exactly: `"no content captured — set OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true and OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental"`.

## Rationale
Surfaces the precise opt-in flags the user must enable on the Copilot CLI to capture content.

## Derived from
- [[Tool Call Inspector]]
