//! Per-`(WsKind, WsEntity)` ring buffer with wildcard subscriber support.
//! Mirrors `web/src/state/live.ts`.

use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;

use crate::tui::model::{WsEntity, WsEnvelope, WsKind};

/// Hard cap per `(kind, entity)` ring.
pub const RING_MAX: usize = 500;

#[derive(Default)]
pub struct LiveFeed {
    rings: RwLock<HashMap<(WsKind, WsEntity), VecDeque<WsEnvelope>>>,
    /// Wildcard envelopes — every envelope is also appended here, capped.
    wildcard: RwLock<VecDeque<WsEnvelope>>,
}

impl LiveFeed {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingest one envelope. Returns the `(kind, entity)` key so the caller
    /// can plumb it through the cache invalidation table.
    pub fn ingest(&self, env: WsEnvelope) -> (WsKind, WsEntity) {
        let key = (env.kind, env.entity);
        {
            let mut rings = self.rings.write().unwrap();
            let ring = rings.entry(key).or_default();
            ring.push_front(env.clone());
            while ring.len() > RING_MAX {
                ring.pop_back();
            }
        }
        {
            let mut w = self.wildcard.write().unwrap();
            w.push_front(env);
            while w.len() > RING_MAX {
                w.pop_back();
            }
        }
        key
    }

    /// Snapshot of the ring for `(kind, entity)` (newest first).
    pub fn snapshot(&self, kind: WsKind, entity: WsEntity) -> Vec<WsEnvelope> {
        self.rings
            .read()
            .unwrap()
            .get(&(kind, entity))
            .map(|d| d.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Wildcard snapshot (every envelope across every key, newest first).
    pub fn wildcard_snapshot(&self) -> Vec<WsEnvelope> {
        self.wildcard.read().unwrap().iter().cloned().collect()
    }

    pub fn len(&self, kind: WsKind, entity: WsEntity) -> usize {
        self.rings
            .read()
            .unwrap()
            .get(&(kind, entity))
            .map(VecDeque::len)
            .unwrap_or(0)
    }

    pub fn wildcard_len(&self) -> usize {
        self.wildcard.read().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn env(kind: WsKind, entity: WsEntity) -> WsEnvelope {
        WsEnvelope {
            kind,
            entity,
            payload: json!({}),
        }
    }

    #[test]
    fn ring_caps_at_500() {
        let f = LiveFeed::new();
        for _ in 0..600 {
            f.ingest(env(WsKind::Span, WsEntity::Span));
        }
        assert_eq!(f.len(WsKind::Span, WsEntity::Span), 500);
    }

    #[test]
    fn wildcard_receives_every_kind() {
        let f = LiveFeed::new();
        f.ingest(env(WsKind::Span, WsEntity::Span));
        f.ingest(env(WsKind::Derived, WsEntity::ChatTurn));
        f.ingest(env(WsKind::Trace, WsEntity::Trace));
        assert_eq!(f.wildcard_len(), 3);
        assert_eq!(f.len(WsKind::Span, WsEntity::Span), 1);
        assert_eq!(f.len(WsKind::Derived, WsEntity::ChatTurn), 1);
        assert_eq!(f.len(WsKind::Trace, WsEntity::Trace), 1);
    }

    #[test]
    fn snapshot_is_newest_first() {
        let f = LiveFeed::new();
        for i in 0..3 {
            f.ingest(WsEnvelope {
                kind: WsKind::Span,
                entity: WsEntity::Span,
                payload: json!({"i": i}),
            });
        }
        let snap = f.snapshot(WsKind::Span, WsEntity::Span);
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].payload, json!({"i": 2}));
        assert_eq!(snap[2].payload, json!({"i": 0}));
    }
}
