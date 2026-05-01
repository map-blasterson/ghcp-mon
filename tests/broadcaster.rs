//! Tests for Broadcaster fan-out. LLR:
//! - Broadcaster fan out via tokio broadcast channel

use ghcp_mon::ws::{Broadcaster, EventMsg};
use serde_json::json;

#[tokio::test]
async fn new_with_capacity_and_send_to_subscribers_fans_out() {
    let bus = Broadcaster::new(8);
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();
    let msg = EventMsg::raw("span", json!({"x": 1}));
    bus.send(msg.clone());
    let r1 = rx1.recv().await.expect("rx1 receives");
    let r2 = rx2.recv().await.expect("rx2 receives");
    assert_eq!(r1.kind, msg.kind);
    assert_eq!(r1.entity, msg.entity);
    assert_eq!(r1.payload, msg.payload);
    assert_eq!(r2.kind, msg.kind);
    assert_eq!(r2.payload, msg.payload);
}

#[tokio::test]
async fn send_with_no_subscribers_does_not_panic_or_error() {
    let bus = Broadcaster::new(4);
    // Public API returns `()` — exercising it with no receivers MUST be a no-op.
    bus.send(EventMsg::raw("span", json!({"a": 1})));
    bus.send(EventMsg::derived("session", json!({"b": 2})));
    // If we got here without panicking, the requirement holds.
}

#[tokio::test]
async fn capacity_passed_to_underlying_broadcast_channel() {
    // Construct with cap=2; fill the buffer with 3 messages and observe Lagged on a slow rx.
    use tokio::sync::broadcast::error::RecvError;
    let bus = Broadcaster::new(2);
    let mut rx = bus.subscribe();
    bus.send(EventMsg::raw("a", json!({})));
    bus.send(EventMsg::raw("b", json!({})));
    bus.send(EventMsg::raw("c", json!({})));
    // The first recv should observe Lagged because rx fell behind on a cap=2 channel.
    match rx.recv().await {
        Err(RecvError::Lagged(_)) => {}
        other => panic!("expected Lagged, got {:?}", other),
    }
}
