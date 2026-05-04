---
type: HLR
tags:
  - req/hlr
  - domain/tool-detail
---
When the user selects an `execute_tool` or `external_tool` span, the dashboard renders a tool-call detail view with specialized layouts for the well-known Copilot tools (`edit`, `view`, `task`, `read_agent`) and a generic fallback for everything else.

## Derived LLRs
- [[Tool detail requires tool call projection]]
- [[Edit tool renders old new with syntax highlight]]
- [[View tool splits line numbers into gutter]]
- [[Task tool renders prompt as markdown]]
- [[Read agent tool renders result as markdown]]
- [[Generic tool renders args splitting code-ish strings]]
- [[Tool detail empty state when no content captured]]
- [[Tool detail body blocks wrap in TextBlock for search]]
- [[Code block highlights via Prism with extension map]]
- [[JsonView pretty prints with optional collapse]]
- [[Detail columns pass span search query to TextBlocks]]
