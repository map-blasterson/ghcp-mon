//! Tests for the WebSocket handler at /ws/events. LLRs:
//! - WS sends hello on connect
//! - WS forwards broadcast events to client
//! - WS responds to ping with pong
//! - WS closes on client close

use futures_util::{SinkExt, StreamExt};
use ghcp_mon::db;
use ghcp_mon::server::{api_router, AppState};
use ghcp_mon::ws::{Broadcaster, EventMsg};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

async fn fresh_state() -> AppState {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("ghcp-mon-ws-{}-{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pool = db::open(&dir.join("test.db")).await.unwrap();
    AppState { pool, bus: Broadcaster::new(64), session_state_dir_override: Arc::new(None) }
}

/// Bind to an ephemeral port and serve the api_router. Returns (state, ws_url).
async fn spawn_server() -> (AppState, String) {
    let state = fresh_state().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = api_router(state.clone());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let url = format!("ws://{}/ws/events", addr);
    (state, url)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sends_hello_on_connect() {
    let (_state, url) = spawn_server().await;
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.expect("connect");
    let first = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next())
        .await.expect("hello within 2s")
        .expect("frame").expect("not error");
    let txt = match first { Message::Text(s) => s, m => panic!("expected text, got {:?}", m) };
    let v: Value = serde_json::from_str(&txt).expect("hello JSON");
    assert_eq!(v["kind"], json!("hello"));
    assert_eq!(v["entity"], json!("control"));
    assert_eq!(v["payload"]["server"], json!("ghcp-mon"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forwards_broadcast_events_to_client() {
    let (state, url) = spawn_server().await;
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.expect("connect");
    // Drain hello.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next()).await
        .unwrap().unwrap().unwrap();
    // Wait briefly so the server task has subscribed before we send.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let msg = EventMsg::raw("span", json!({"x": 1, "y": "two"}));
    state.bus.send(msg.clone());
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next())
        .await.expect("forwarded frame within 2s")
        .expect("frame").expect("not error");
    let txt = match frame { Message::Text(s) => s, m => panic!("expected text, got {:?}", m) };
    let v: Value = serde_json::from_str(&txt).unwrap();
    assert_eq!(v["kind"], json!("span"));
    assert_eq!(v["entity"], json!("span"));
    assert_eq!(v["payload"]["x"], json!(1));
    assert_eq!(v["payload"]["y"], json!("two"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responds_to_ping_with_pong_echoing_payload() {
    let (_state, url) = spawn_server().await;
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.expect("connect");
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next()).await
        .unwrap().unwrap().unwrap(); // hello
    let payload: Vec<u8> = b"ping-payload-xyz".to_vec();
    ws.send(Message::Ping(payload.clone().into())).await.expect("send ping");

    // Look for a Pong with the matching payload (skip any forwarded frames in between).
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() { panic!("timed out waiting for Pong"); }
        let frame = tokio::time::timeout(remaining, ws.next()).await
            .expect("frame within deadline").expect("some").expect("ok");
        if let Message::Pong(p) = frame {
            assert_eq!(p.as_ref(), payload.as_slice(), "Pong MUST echo Ping payload");
            return;
        }
        // Skip text/binary/other.
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exits_loop_when_client_sends_close() {
    let (state, url) = spawn_server().await;
    let (mut ws, _resp) = tokio_tungstenite::connect_async(&url).await.expect("connect");
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next()).await
        .unwrap().unwrap().unwrap();
    ws.send(Message::Close(None)).await.expect("send close");
    // Drain frames until stream closes; this MUST happen quickly (server exits its loop).
    let drained = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while let Some(_) = ws.next().await {}
    }).await;
    assert!(drained.is_ok(), "server-side loop MUST exit when client closes");
    // Sanity: bus is still usable post-close (the dropped subscriber is just gone).
    state.bus.send(EventMsg::raw("span", json!({})));
}
