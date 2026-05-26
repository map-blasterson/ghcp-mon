---
type: HLR
tags:
  - req/hlr
  - domain/tool-detail
---
When the user selects an `execute_tool` or `external_tool` span, the dashboard renders a tool-call detail view with specialized layouts for well-known tool kinds (`edit`, `read`, `task`, `read_agent`) and a generic fallback for everything else.

## Normalized tool-call vocabulary
UI-rendering LLRs under this HLR are specified over a normalized tool-call shape rather than raw OTLP fields. **Normalized tool kinds** include `read`, `write`, `edit`, `patch`, and `shell`. **Normalized arguments** are referred to as: *file-path*, *old-text*, *new-text*, *body-text*, *shell-command*, *target-url*, *patch-text*. **Normalized result envelope** has four optional fields: `body_string` (verbatim string result), `output_text`, `diff_text`, `metadata`. The active **vendor adapter** is selected per-span by `service_name`; concrete mappings live in each vendor scope (e.g., [[Copilot tool-call shape]], [[opencode tool-call shape]]).

## Derived LLRs
- [[Tool detail requires tool call projection]]
- [[Edit tool renders old new with syntax highlight]]
- [[Edit tool result renders unified diff from metadata]]
- [[View tool splits line numbers into gutter]]
- [[Task tool renders prompt as markdown]]
- [[Read agent tool renders result as markdown]]
- [[Generic tool renders args splitting code-ish strings]]
- [[Tool detail empty state when no content captured]]
- [[Tool detail body blocks wrap in TextBlock for search]]
- [[Code block highlights via Prism with extension map]]
- [[JsonView pretty prints with optional collapse]]
- [[Detail columns pass span search query to TextBlocks]]
- [[Tool detail metadata panel collapsible]]
- [[Tool detail hero panel surfaces key argument]]
