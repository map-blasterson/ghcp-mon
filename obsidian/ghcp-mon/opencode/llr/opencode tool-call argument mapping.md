---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/opencode
---
When `service_name === "opencode"`, the opencode tool-call adapter SHALL resolve normalized argument concepts to raw `gen_ai.tool.call.arguments` fields as follows: **file-path** = `args.filePath`; **old-text** = `args.oldString`; **new-text** = `args.newString`; **body-text** = `args.content`; **shell-command** = `args.command`; **target-url** = `args.url`. Concepts not listed MUST remain unresolved (treated as absent).

## Rationale
Concentrates all opencode-specific raw field names in one place so UI LLRs can refer to normalized concepts without enumerating field aliases.

## Derived from
- [[opencode tool-call shape]]
