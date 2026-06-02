---
type: LLR
tags:
  - req/llr
  - tui
  - domain/architecture
---
The `RawBrowser` scenario MUST be implemented as a `Scenario` trait object whose `draw` delegates to `scenarios::render_placeholder(area, buf, ScenarioType::RawBrowser, config)` and whose `handle_key`, `keymap_entries` return empty / pass. This eliminates the "fall through to placeholder" arm in `App::draw_workspace`.

## Rationale
Every column type goes through the same dispatch path; no special-case rendering branch.

## Derived from
- [[Per-Scenario Trait Architecture]]
