//! WebSocket bus singleton. Mirrors `web/src/api/ws.ts`:
//!
//! - Lazy `start()` opens the socket only on first call.
//! - Exponential backoff `min(30_000, 500 * 2^attempt)` ms.
//! - `attempt` resets to 0 on successful open.
//! - Malformed JSON frames are swallowed (logged at debug level only).
//! - `error` event triggers `close()` so reconnect goes through the standard
//!   path.
//! - After ≥5 consecutive reconnect failures the status changes from
//!   `Reconnecting` to `Error`.

use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

use crate::tui::model::WsEnvelope;

/// Connection status surfaced to the top-bar dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsStatus {
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

/// Singleton bus. Cheap to clone; the underlying socket runs in a background
/// task spawned by [`WsBus::start`].
#[derive(Clone)]
pub struct WsBus {
    inner: Arc<Inner>,
}

struct Inner {
    ws_url: String,
    status: RwLock<WsStatus>,
    started: RwLock<bool>,
    envelopes: broadcast::Sender<WsEnvelope>,
    status_changes: broadcast::Sender<WsStatus>,
}

impl WsBus {
    /// Wrap a normalised WS URL. `start()` is a no-op until called.
    pub fn new(ws_url: String) -> Self {
        let (env_tx, _) = broadcast::channel::<WsEnvelope>(1024);
        let (st_tx, _) = broadcast::channel::<WsStatus>(64);
        Self {
            inner: Arc::new(Inner {
                ws_url,
                status: RwLock::new(WsStatus::Connecting),
                started: RwLock::new(false),
                envelopes: env_tx,
                status_changes: st_tx,
            }),
        }
    }

    /// Open the WS on first call. Subsequent calls are no-ops.
    pub fn start(&self) {
        {
            let mut started = self.inner.started.write().unwrap();
            if *started {
                return;
            }
            *started = true;
        }
        let me = self.clone();
        tokio::spawn(async move { me.run().await });
    }

    /// Current snapshot.
    pub fn is_connected(&self) -> bool {
        matches!(*self.inner.status.read().unwrap(), WsStatus::Connected)
    }

    /// Status snapshot.
    pub fn status(&self) -> WsStatus {
        *self.inner.status.read().unwrap()
    }

    /// Subscribe to envelopes. Receivers are independent; each gets every
    /// future envelope.
    pub fn subscribe(&self) -> broadcast::Receiver<WsEnvelope> {
        self.inner.envelopes.subscribe()
    }

    /// Subscribe to status changes.
    pub fn on_status(&self) -> broadcast::Receiver<WsStatus> {
        self.inner.status_changes.subscribe()
    }

    fn set_status(&self, s: WsStatus) {
        {
            let mut st = self.inner.status.write().unwrap();
            *st = s;
        }
        let _ = self.inner.status_changes.send(s);
    }

    async fn run(self) {
        let mut attempt: u32 = 0;
        let mut failures: u32 = 0;
        loop {
            self.set_status(if attempt == 0 {
                WsStatus::Connecting
            } else if failures >= 5 {
                WsStatus::Error
            } else {
                WsStatus::Reconnecting
            });

            match connect_async(&self.inner.ws_url).await {
                Ok((mut stream, _)) => {
                    info!(url = %self.inner.ws_url, "tui ws connected");
                    self.set_status(WsStatus::Connected);
                    attempt = 0;
                    failures = 0;

                    while let Some(msg) = stream.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                match serde_json::from_str::<WsEnvelope>(&text) {
                                    Ok(env) => {
                                        let _ = self.inner.envelopes.send(env);
                                    }
                                    Err(e) => {
                                        debug!(error = %e, "tui ws malformed json ignored");
                                    }
                                }
                            }
                            Ok(Message::Binary(_)) => {}
                            Ok(Message::Ping(p)) => {
                                let _ = stream.send(Message::Pong(p)).await;
                            }
                            Ok(Message::Close(_)) | Ok(Message::Pong(_)) | Ok(Message::Frame(_)) => {}
                            Err(e) => {
                                // Mirror the web client: on any socket error,
                                // close so the standard reconnect path runs.
                                debug!(error = %e, "tui ws socket error -> close");
                                let _ = stream.close(None).await;
                                break;
                            }
                        }
                    }
                    warn!("tui ws closed; will reconnect");
                }
                Err(e) => {
                    failures = failures.saturating_add(1);
                    debug!(attempt, failures, error = %e, "tui ws connect failed");
                }
            }

            let delay_ms = backoff_ms(attempt);
            attempt = attempt.saturating_add(1);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
    }
}

/// `min(30_000, 500 * 2^attempt)` ms, saturating on overflow.
pub fn backoff_ms(attempt: u32) -> u64 {
    let raw: u64 = 500u64.saturating_mul(1u64 << attempt.min(20));
    raw.min(30_000)
}

/// Normalize a `--server` URL:
/// - Strip trailing `/`.
/// - Reject schemes other than `http` / `https`.
/// - Return a `(rest_base, ws_url)` pair where `ws_url` ends in `/ws/events`.
pub fn normalize_server(server: &str) -> Result<(String, String)> {
    let trimmed = server.trim_end_matches('/');
    let (scheme, rest) = if let Some(rest) = trimmed.strip_prefix("http://") {
        ("http", rest)
    } else if let Some(rest) = trimmed.strip_prefix("https://") {
        ("https", rest)
    } else {
        return Err(anyhow!(
            "--server must start with http:// or https:// (got {server:?})"
        ));
    };
    let ws_scheme = if scheme == "http" { "ws" } else { "wss" };
    let rest_base = format!("{scheme}://{rest}");
    let ws_url = format!("{ws_scheme}://{rest}/ws/events");
    Ok((rest_base, ws_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_schedule() {
        assert_eq!(backoff_ms(0), 500);
        assert_eq!(backoff_ms(1), 1_000);
        assert_eq!(backoff_ms(2), 2_000);
        assert_eq!(backoff_ms(3), 4_000);
        assert_eq!(backoff_ms(4), 8_000);
        assert_eq!(backoff_ms(5), 16_000);
        assert_eq!(backoff_ms(6), 30_000); // capped
        assert_eq!(backoff_ms(50), 30_000); // saturated
    }

    #[test]
    fn normalize_server_http_to_ws() {
        let (rest, ws) = normalize_server("http://127.0.0.1:4319").unwrap();
        assert_eq!(rest, "http://127.0.0.1:4319");
        assert_eq!(ws, "ws://127.0.0.1:4319/ws/events");
    }

    #[test]
    fn normalize_server_https_to_wss() {
        let (rest, ws) = normalize_server("https://example.com/").unwrap();
        assert_eq!(rest, "https://example.com");
        assert_eq!(ws, "wss://example.com/ws/events");
    }

    #[test]
    fn normalize_server_strips_trailing_slashes() {
        let (rest, ws) = normalize_server("http://h:1/").unwrap();
        assert_eq!(rest, "http://h:1");
        assert_eq!(ws, "ws://h:1/ws/events");
    }

    #[test]
    fn normalize_server_rejects_other_schemes() {
        assert!(normalize_server("ftp://x").is_err());
        assert!(normalize_server("ws://x").is_err());
        assert!(normalize_server("no-scheme").is_err());
    }
}
