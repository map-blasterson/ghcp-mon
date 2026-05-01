use axum::{extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State}, response::IntoResponse};
use crate::server::AppState;
use tracing::debug;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| client_loop(socket, state))
}

async fn client_loop(mut socket: WebSocket, state: AppState) {
    let mut rx = state.bus.subscribe();
    let hello = serde_json::json!({"kind":"hello","entity":"control","payload":{"server":"ghcp-mon"}});
    if socket.send(Message::Text(hello.to_string())).await.is_err() { return; }
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(m) => {
                        let txt = serde_json::to_string(&m).unwrap_or_default();
                        if socket.send(Message::Text(txt)).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!("ws client lagged {n}");
                    }
                    Err(_) => break,
                }
            }
            client = socket.recv() => {
                match client {
                    Some(Ok(Message::Ping(p))) => { let _ = socket.send(Message::Pong(p)).await; }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
