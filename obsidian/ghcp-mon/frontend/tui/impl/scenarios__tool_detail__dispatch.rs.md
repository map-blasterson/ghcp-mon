---
type: impl
source: src/tui/scenarios/tool_detail/dispatch.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Renderer dispatch: `route(detail) -> Route{Empty(NotATool)|Native(RendererKind)|External}` (native `tool_call` preferred over `external_tool_call`), and `native_renderer(tool_name)` mapping edit/write→Edit, read→View, `task`→Task, `read_agent`→ReadAgent, else Generic, via the Copilot `tool_name_mapping` kind table. Pure decision logic, no rendering.

## Source For
- [[Tool detail requires tool call projection]]
- [[Tool detail prefers native tool call over external]]
- [[Edit tool renders old new with syntax highlight]]
- [[View tool splits line numbers into gutter]]
- [[Task tool renders prompt as markdown]]
- [[Read agent tool renders result as markdown]]
- [[Generic tool renders args splitting code-ish strings]]
- [[External tool detail uses generic args renderer]]
