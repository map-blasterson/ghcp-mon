---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/copilot
---
When `service_name === "github-copilot"`, the Copilot tool-call adapter SHALL resolve normalized argument concepts to raw `gen_ai.tool.call.arguments` fields as follows: **file-path** = `args.path`; **old-text** = `args.old_str`; **new-text** = `args.new_str`; **body-text** = `args.file_text`; **shell-command** = `args.command`; **target-url** = `args.url`; **patch-text** = `args.patch` when a string, otherwise `args.input` when a string, otherwise the raw `args` value when `args` itself is a string. Concepts not listed MUST remain unresolved (treated as absent).

## Rationale
Concentrates all Copilot-specific raw field names in one place so UI LLRs can refer to normalized concepts without enumerating field aliases.

## Derived from
- [[Copilot tool-call shape]]
