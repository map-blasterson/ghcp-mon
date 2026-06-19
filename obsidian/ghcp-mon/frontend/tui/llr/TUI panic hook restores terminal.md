---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
Before entering the draw loop, `tui::run` MUST install a panic hook that calls `ratatui::restore()` before delegating to the previous hook. The hook MUST be set before `ratatui::try_init` so a crash during the run leaves the terminal in cooked mode and visible.

## Rationale
A panicking TUI must not leave the terminal blank with input echo off.

## Derived from
- [[Server URL Normalization and Reconnect Status]]
