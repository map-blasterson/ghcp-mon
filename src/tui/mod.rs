//! TUI subcommand entry point. `pub async fn run(server, mouse)` is invoked
//! by `main.rs` after the tracing subscriber is installed.
//!
//! Submodules:
//! - [`api`] — async REST client.
//! - [`app`] — top-level state + draw + event loop.
//! - [`cache`] — query cache (TanStack-Query analog).
//! - [`event`] — bounded mpsc + coalesced WS-tick.
//! - [`format`] — fmt_ns, fmt_clock, hash_color (FNV-1a → terminal RGB).
//! - [`live_feed`] — per-(kind,entity) ring buffer.
//! - [`model`] — serde port of `web/src/api/types.ts`.
//! - [`persist`] — TOML workspace load/save + migrate-drop.
//! - [`scenarios`] — Phase 0 placeholders.
//! - [`vendor`] — Copilot tool-call adapter.
//! - [`widgets`] — status dot, kind badge, log overlay.
//! - [`workspace`] — Columns / context-widget state.
//! - [`ws`] — WebSocket bus singleton.

use anyhow::Result;
use ratatui::crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use ratatui::crossterm::execute;

pub mod api;
pub mod app;
pub mod cache;
pub mod event;
pub mod format;
pub mod live_feed;
pub mod model;
pub mod persist;
pub mod scenarios;
pub mod vendor;
pub mod widgets;
pub mod workspace;
pub mod ws;

pub use widgets::log_overlay::LogBuffer;

/// TUI entry point. Reachable from `main.rs` under the `Attach` subcommand.
pub async fn run(server: &str, mouse: bool, log_buffer: LogBuffer) -> Result<()> {
    let (rest_base, ws_url) = ws::normalize_server(server)?;
    tracing::info!(rest_base = %rest_base, ws_url = %ws_url, "tui starting");

    let api = api::ApiClient::new(rest_base);
    let ws = ws::WsBus::new(ws_url);
    ws.start();

    // Panic-hook guard — restore the terminal before printing.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        original_hook(info);
    }));

    let mut terminal = ratatui::try_init()?;
    if mouse {
        let _ = execute!(std::io::stdout(), EnableMouseCapture);
    }

    let mut app = app::App::new(api, ws.clone(), log_buffer, mouse);
    let (_tx, rx) = app::spawn_event_sources(&ws);

    let result = app::event_loop(&mut terminal, &mut app, rx).await;

    // Clean teardown.
    if app.mouse_enabled {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
    }
    ratatui::restore();

    // Persist on exit.
    let _ = persist::save(&app.workspace);
    result
}
