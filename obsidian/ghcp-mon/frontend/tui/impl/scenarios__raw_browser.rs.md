---
type: impl
source: src/tui/scenarios/raw_browser.rs
lang: rust
tags:
  - impl/original
  - impl/rust
---
`RawBrowserScenario` — placeholder scenario behind the `Scenario` trait. `draw` delegates to `scenarios::render_placeholder(area, buf, ScenarioType::RawBrowser, config)`; `handle_key` returns `KeyOutcome::pass()`; `keymap_entries` returns empty. No state.

## Source For
- [[TUI RawBrowser scenario is placeholder behind trait]]
