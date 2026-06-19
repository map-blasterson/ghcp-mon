---
type: LLR
tags:
  - req/llr
  - tui
  - domain/event-loop
---
`tui::app::event_loop` MUST drive the TUI from a single `tokio::select!` racing over exactly five sources: the WS envelope broadcast (`WsBus::subscribe`), the WS status broadcast (`WsBus::on_status`), the async crossterm `EventStream`, the query cache's `Notify` handle (`QueryCache::changed_handle`), and one optional animation-deadline sleep (`sleep_until_anim(App::next_anim_deadline_ms(spinner_visible), now_ms)`). The loop MUST NOT use a heartbeat tick or a dedicated mpsc `AppEvent` channel. When the deadline is `None`, the animation arm MUST park indefinitely (via `std::future::pending`).

## Rationale
Event-driven loop — a fully idle TUI parks until something happens, rather than waking every frame to do nothing.

## Derived from
- [[Terminal Event Loop]]
