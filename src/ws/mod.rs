use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventMsg {
    pub kind: String,    // "span" | "metric" | "log" | "derived"
    pub entity: String,  // free-form: "tool_call", "turn", "session", ...
    pub payload: Value,
}

impl EventMsg {
    pub fn raw(kind: &str, payload: Value) -> Self {
        Self { kind: kind.into(), entity: kind.into(), payload }
    }
    pub fn derived(entity: &str, payload: Value) -> Self {
        Self { kind: "derived".into(), entity: entity.into(), payload }
    }
}

#[derive(Clone)]
pub struct Broadcaster {
    tx: broadcast::Sender<EventMsg>,
}

impl Broadcaster {
    pub fn new(cap: usize) -> Self {
        let (tx, _) = broadcast::channel(cap);
        Self { tx }
    }
    pub fn subscribe(&self) -> broadcast::Receiver<EventMsg> { self.tx.subscribe() }
    pub fn send(&self, msg: EventMsg) {
        let _ = self.tx.send(msg);
    }
}

pub mod handler;
