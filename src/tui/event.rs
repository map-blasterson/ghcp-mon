//! Event channel + coalesced WS-tick dispatcher.
//!
//! The draw loop reads from a single bounded `mpsc::Receiver<AppEvent>` of
//! capacity 256. WS envelopes are translated to dirty `QueryKey` prefixes via
//! [`crate::tui::cache::ws_invalidation_prefixes`] and coalesced into one
//! [`AppEvent::WsTick`] per ratatui-frame interval (≤16 ms).

use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

use crate::tui::model::WsEnvelope;

/// Per-frame interval used for coalescing WS ticks (~60 fps).
pub const FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Cache key prefix segments (each `String` corresponds to one segment).
pub type DirtyPrefix = Vec<String>;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    Crossterm(crossterm::event::Event),
    WsTick {
        /// Cache-key prefixes invalidated since the last tick. Deduped on
        /// emit; one event per frame at most.
        dirty_prefixes: Vec<DirtyPrefix>,
        /// Mirror envelopes — handed to the live feed by the receiver.
        envelopes: Vec<WsEnvelope>,
    },
    /// Reserved for Phase 1+ HTTP fetch completions.
    QueryResult {
        key: Vec<String>,
        generation: u64,
        value: serde_json::Value,
    },
    Quit,
}

/// Channel pair. Senders are cheap to clone.
pub fn channel() -> (mpsc::Sender<AppEvent>, mpsc::Receiver<AppEvent>) {
    mpsc::channel(256)
}

/// Spawn the WS coalescer. Subscribes to `ws_rx`, drains envelopes into the
/// live feed, translates them to dirty cache-key prefixes via the WS
/// invalidation table, and emits one [`AppEvent::WsTick`] per
/// [`FRAME_INTERVAL`] carrying the union of dirty prefixes + the raw
/// envelopes (the latter are appended to the live feed by the receiver).
pub fn spawn_ws_coalescer(
    mut ws_rx: broadcast::Receiver<WsEnvelope>,
    tx: mpsc::Sender<AppEvent>,
) {
    tokio::spawn(async move {
        let mut buf_envelopes: Vec<WsEnvelope> = Vec::new();
        let mut buf_prefixes: Vec<DirtyPrefix> = Vec::new();
        let mut deadline = tokio::time::Instant::now() + FRAME_INTERVAL;
        loop {
            tokio::select! {
                msg = ws_rx.recv() => match msg {
                    Ok(env) => {
                        for p in crate::tui::cache::ws_invalidation_prefixes(env.kind, env.entity) {
                            let v: DirtyPrefix = p.iter().map(|s| (*s).to_string()).collect();
                            if !buf_prefixes.contains(&v) {
                                buf_prefixes.push(v);
                            }
                        }
                        buf_envelopes.push(env);
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(lagged = n, "ws coalescer lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                _ = tokio::time::sleep_until(deadline) => {
                    if !buf_envelopes.is_empty() || !buf_prefixes.is_empty() {
                        let ev = AppEvent::WsTick {
                            dirty_prefixes: std::mem::take(&mut buf_prefixes),
                            envelopes: std::mem::take(&mut buf_envelopes),
                        };
                        if tx.send(ev).await.is_err() {
                            break;
                        }
                    }
                    deadline = tokio::time::Instant::now() + FRAME_INTERVAL;
                }
            }
        }
    });
}

/// Spawn the crossterm event reader (blocking poll in a dedicated task).
pub fn spawn_crossterm_reader(tx: mpsc::Sender<AppEvent>) {
    tokio::task::spawn_blocking(move || {
        loop {
            if tx.is_closed() {
                break;
            }
            // 50 ms poll cadence keeps the task responsive to shutdown.
            match crossterm::event::poll(Duration::from_millis(50)) {
                Ok(true) => match crossterm::event::read() {
                    Ok(ev) => {
                        if tx.blocking_send(AppEvent::Crossterm(ev)).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                },
                Ok(false) => {}
                Err(_) => break,
            }
        }
    });
}

/// Spawn the 16 ms animation tick.
pub fn spawn_tick(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(FRAME_INTERVAL);
        loop {
            interval.tick().await;
            if tx.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });
}
