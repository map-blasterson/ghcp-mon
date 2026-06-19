//! Async query cache (TanStack-Query analog) for the TUI.
//!
//! ## Contract (from the §"Query cache contract" of the session plan)
//!
//! 1. **In-flight dedupe** — a fetch in progress for key K causes a
//!    duplicate `get(K)` to await the same future.
//! 2. **Generation-based stale suppression** — every fetch carries a
//!    monotonic per-key `generation`; on completion, results from
//!    generations older than the latest dispatched are discarded.
//! 3. **Prefix invalidation** — `invalidate(prefix)` matches keys whose
//!    initial segments equal `prefix`; matched keys are marked stale and
//!    their generation bumped so the next `get` triggers a refetch and
//!    older in-flights are dropped.
//! 4. **Stale-while-revalidate** — `get` returns the cached value
//!    immediately and dispatches a refetch when the value is past its
//!    `stale_after`.
//! 5. **LRU eviction** — capped at 1024 entries for keys whose first
//!    segment is `"span"`. Other key classes are small and uncapped.
//!
//! The WS → invalidation table lives in [`ws_invalidation_prefixes`].

use std::collections::HashMap;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::sync::{Notify, broadcast};

use crate::tui::model::{WsEntity, WsKind};

/// Query key — a vector of literal segments. Designed for prefix matching.
pub type QueryKey = Vec<String>;

/// Helper to build a [`QueryKey`] from string slices.
pub fn qkey<I, S>(parts: I) -> QueryKey
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    parts.into_iter().map(Into::into).collect()
}

#[derive(Debug, Clone)]
pub struct CachedValue {
    pub value: Value,
    pub generation: u64,
    pub fetched_at: Instant,
    pub stale_after: Duration,
}

impl CachedValue {
    pub fn is_stale(&self) -> bool {
        self.fetched_at.elapsed() >= self.stale_after
    }
}

#[derive(Debug, Clone)]
pub struct CacheGet {
    pub value: Option<CachedValue>,
    pub will_refetch: bool,
}

/// Span-key LRU cap (mirrors the plan).
pub const SPAN_LRU_CAP: usize = 1024;

struct Entry {
    cached: Option<CachedValue>,
    /// Latest dispatched generation. Result completions older than this are
    /// discarded.
    latest_gen: u64,
    /// In-flight receiver, shared across duplicate `get` callers.
    in_flight: Option<broadcast::Sender<Value>>,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            cached: None,
            latest_gen: 0,
            in_flight: None,
        }
    }
}

#[derive(Default)]
pub struct QueryCache {
    inner: RwLock<Inner>,
    /// Wake-once signal fired whenever a background fetch completes (or an
    /// invalidation flips a cached value to stale) so the event loop can
    /// redraw without polling. Lossless under bursts because
    /// [`Notify::notify_one`] stores at most one permit — many events
    /// collapse to one wakeup that then drains everything.
    ///
    /// Held as `Arc` so the event loop can keep an owned handle and avoid
    /// borrowing `&self` for the lifetime of the select-arm future.
    changed: Arc<Notify>,
}

impl QueryCache {
    /// Owned handle to the "cache value changed" wakeup. Cheap to clone.
    pub fn changed_handle(&self) -> Arc<Notify> {
        self.changed.clone()
    }
}

#[derive(Default)]
struct Inner {
    entries: HashMap<QueryKey, Entry>,
    /// FIFO of `["span", ...]` keys for LRU eviction.
    span_lru: VecDeque<QueryKey>,
}

/// Result returned by a fetcher closure. Carries the key and generation
/// stamp so completions can be matched against the cache state.
pub struct FetchedRecord {
    pub key: QueryKey,
    pub generation: u64,
    pub value: Value,
    pub stale_after: Duration,
}

pub type Fetcher = Box<
    dyn FnOnce() -> Pin<Box<dyn Future<Output = anyhow::Result<Value>> + Send>> + Send + 'static,
>;

impl QueryCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read-only lookup with stale-while-revalidate semantics. Returns the
    /// cached value (if any) and a flag indicating whether a refetch is
    /// needed.
    pub fn peek(&self, key: &QueryKey) -> CacheGet {
        let g = self.inner.read().unwrap();
        match g.entries.get(key) {
            Some(e) => {
                let stale = e.cached.as_ref().map(|c| c.is_stale()).unwrap_or(true);
                let in_flight = e.in_flight.is_some();
                CacheGet {
                    value: e.cached.clone(),
                    will_refetch: stale && !in_flight,
                }
            }
            None => CacheGet {
                value: None,
                will_refetch: true,
            },
        }
    }

    /// Insert a fresh value (e.g. on `QueryResult` event).
    /// Older-generation completions are dropped per the contract.
    pub fn put(&self, rec: FetchedRecord) {
        {
            let mut g = self.inner.write().unwrap();
            let entry = g.entries.entry(rec.key.clone()).or_default();
            if rec.generation < entry.latest_gen {
                // Older than the most-recent dispatch — discard.
                return;
            }
            entry.cached = Some(CachedValue {
                value: rec.value,
                generation: rec.generation,
                fetched_at: Instant::now(),
                stale_after: rec.stale_after,
            });
            entry.in_flight = None;
            // LRU bookkeeping for span keys only.
            if rec.key.first().map(String::as_str) == Some("span") {
                // Move-to-front by remove + push_back.
                g.span_lru.retain(|k| k != &rec.key);
                g.span_lru.push_back(rec.key.clone());
                while g.span_lru.len() > SPAN_LRU_CAP {
                    if let Some(oldest) = g.span_lru.pop_front() {
                        g.entries.remove(&oldest);
                    }
                }
            }
        }
        self.changed.notify_one();
    }

    /// Begin a new fetch generation for `key`. Returns the assigned
    /// generation; the caller dispatches the fetcher and posts a
    /// `QueryResult { generation }` event when it completes. If a duplicate
    /// in-flight fetch exists, returns its generation and an `await_recv`
    /// receiver instead of opening a new one.
    pub fn begin_fetch(&self, key: &QueryKey) -> FetchTicket {
        let mut g = self.inner.write().unwrap();
        let entry = g.entries.entry(key.clone()).or_default();
        if let Some(tx) = &entry.in_flight {
            // Dedupe: subscribe to the existing in-flight broadcast.
            return FetchTicket {
                generation: entry.latest_gen,
                already_in_flight: true,
                wait: Some(tx.subscribe()),
            };
        }
        entry.latest_gen = entry.latest_gen.wrapping_add(1);
        let (tx, rx) = broadcast::channel::<Value>(1);
        entry.in_flight = Some(tx);
        FetchTicket {
            generation: entry.latest_gen,
            already_in_flight: false,
            wait: Some(rx),
        }
    }

    /// Mark an in-flight fetch as finished (used to wake duplicate waiters
    /// even when the result was discarded as stale).
    pub fn finish_fetch(&self, key: &QueryKey, value: &Value) {
        {
            let mut g = self.inner.write().unwrap();
            if let Some(entry) = g.entries.get_mut(key) {
                if let Some(tx) = entry.in_flight.take() {
                    let _ = tx.send(value.clone());
                }
            }
        }
        self.changed.notify_one();
    }

    /// Prefix invalidation — bump generation on every matching key and
    /// mark cached values stale so the next `peek` triggers a refetch.
    /// Returns the keys invalidated.
    pub fn invalidate(&self, prefix: &[&str]) -> Vec<QueryKey> {
        let matched = {
            let mut g = self.inner.write().unwrap();
            let mut matched = Vec::new();
            for (k, entry) in g.entries.iter_mut() {
                if key_has_prefix(k, prefix) {
                    entry.latest_gen = entry.latest_gen.wrapping_add(1);
                    if let Some(c) = entry.cached.as_mut() {
                        // Force-stale: set fetched_at far in the past.
                        c.fetched_at =
                            Instant::now() - c.stale_after.saturating_mul(2).max(c.stale_after);
                    }
                    matched.push(k.clone());
                }
            }
            matched
        };
        if !matched.is_empty() {
            self.changed.notify_one();
        }
        matched
    }

    /// For tests only.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().entries.len()
    }
}

/// Convenience wrapper composing `peek` + `begin_fetch` + `put` +
/// `finish_fetch` + in-flight dedupe. Returns a JSON value:
///
/// 1. If the cache holds a fresh value, returns it without dispatching.
/// 2. If a duplicate fetch is already in flight, awaits it.
/// 3. Otherwise, runs `fetcher`, inserts the result with the generation
///    assigned by `begin_fetch`, and returns the result.
///
/// Stale-while-revalidate is **not** applied here — callers that want SWR
/// should `peek` first and pass cached values to the renderer while
/// `cache_get` runs in a spawned task. This function blocks until a value
/// is available.
pub async fn cache_get<F, Fut>(
    cache: &std::sync::Arc<QueryCache>,
    key: QueryKey,
    stale_after: Duration,
    fetcher: F,
) -> anyhow::Result<Value>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = anyhow::Result<Value>>,
{
    // Fast path: fresh cached value.
    {
        let g = cache.peek(&key);
        if let Some(c) = g.value {
            if !c.is_stale() {
                return Ok(c.value);
            }
        }
    }
    // Begin (or join) an in-flight fetch.
    let ticket = cache.begin_fetch(&key);
    if ticket.already_in_flight {
        if let Some(mut rx) = ticket.wait {
            if let Ok(v) = rx.recv().await {
                return Ok(v);
            }
        }
        // Fallback: peek what's in cache.
        let g = cache.peek(&key);
        if let Some(c) = g.value {
            return Ok(c.value);
        }
        return Err(anyhow::anyhow!("in-flight fetch dropped without result"));
    }
    let generation = ticket.generation;
    drop(ticket.wait); // we don't read our own broadcast
    match fetcher().await {
        Ok(value) => {
            cache.put(FetchedRecord {
                key: key.clone(),
                generation,
                value: value.clone(),
                stale_after,
            });
            cache.finish_fetch(&key, &value);
            Ok(value)
        }
        Err(e) => {
            cache.finish_fetch(&key, &Value::Null);
            Err(e)
        }
    }
}

pub struct FetchTicket {
    pub generation: u64,
    pub already_in_flight: bool,
    pub wait: Option<broadcast::Receiver<Value>>,
}

/// Fetch policy for [`swr_read`]: whether to dispatch a background refetch
/// when the cached value is missing or stale.
///
/// * `Swr` — stale-while-revalidate: return cached value (if any) and spawn
///   a background fetch when `peek` reports `will_refetch`.
/// * `ReadOnly` — return only what's already in the cache. Used by render
///   paths whose fetch lifecycle is owned elsewhere (e.g. debounced search
///   per `TUI Spans search input edit semantics`).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FetchPolicy {
    Swr,
    ReadOnly,
}

/// Read the typed value at `key` from `cache`, deserializing into `T`. If
/// `policy` is [`FetchPolicy::Swr`] and the cache reports `will_refetch`,
/// spawn `fetch` in the background under [`cache_get`] so the next render
/// can pick up the fresh value. Deserialization failures are logged at
/// `warn` (with the key) and treated as a cache miss — never silently
/// swallowed.
///
/// Replaces the seven copy-pasted `cached_*` peek+spawn+parse blocks in
/// `app.rs` (per Phase 1 of the rectification plan).
pub fn swr_read<T, F, Fut>(
    cache: &std::sync::Arc<QueryCache>,
    key: QueryKey,
    stale_after: Duration,
    policy: FetchPolicy,
    fetch: F,
) -> Option<T>
where
    T: serde::de::DeserializeOwned,
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<Value>> + Send + 'static,
{
    let g = cache.peek(&key);
    if g.will_refetch && matches!(policy, FetchPolicy::Swr) {
        let cache_c = cache.clone();
        let key_c = key.clone();
        tokio::spawn(async move {
            let _ = cache_get(&cache_c, key_c, stale_after, fetch).await;
        });
    }
    let cached = g.value?;
    match serde_json::from_value::<T>(cached.value) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(
                key = ?key,
                error = %e,
                "swr_read: cached value failed to deserialize"
            );
            None
        }
    }
}

fn key_has_prefix(key: &QueryKey, prefix: &[&str]) -> bool {
    if prefix.len() > key.len() {
        return false;
    }
    for (a, b) in key.iter().zip(prefix.iter()) {
        if a != b {
            return false;
        }
    }
    true
}

/// WS → cache invalidation table. For a given `(kind, entity)` envelope,
/// returns the cache-key prefixes that should be invalidated. Mirrors the
/// table in §"Query cache contract" of the session plan.
pub fn ws_invalidation_prefixes(kind: WsKind, entity: WsEntity) -> Vec<&'static [&'static str]> {
    let mut out: Vec<&'static [&'static str]> = Vec::new();
    let derived_session = matches!(kind, WsKind::Derived) && matches!(entity, WsEntity::Session);
    let derived_chat_turn =
        matches!(kind, WsKind::Derived) && matches!(entity, WsEntity::ChatTurn);
    let derived_tool_call =
        matches!(kind, WsKind::Derived) && matches!(entity, WsEntity::ToolCall);
    let derived_any = matches!(kind, WsKind::Derived);
    let span_span = matches!(kind, WsKind::Span) && matches!(entity, WsEntity::Span);
    let span_placeholder =
        matches!(kind, WsKind::Span) && matches!(entity, WsEntity::Placeholder);
    let trace_trace = matches!(kind, WsKind::Trace) && matches!(entity, WsEntity::Trace);
    let metric_metric = matches!(kind, WsKind::Metric) && matches!(entity, WsEntity::Metric);

    if derived_session || derived_chat_turn {
        out.push(&["sessions"][..]);
    }
    if trace_trace || span_span || span_placeholder || derived_any {
        out.push(&["traces"][..]);
        out.push(&["session-span-tree"][..]);
    }
    if derived_chat_turn || span_span || span_placeholder {
        out.push(&["session-contexts"][..]);
    }
    if span_span || derived_tool_call {
        out.push(&["spans"][..]);
    }
    if span_span || metric_metric {
        out.push(&["raw"][..]);
    }
    if span_span {
        out.push(&["traces-detail"][..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(parts: &[&str]) -> QueryKey {
        parts.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn put_and_peek_returns_value() {
        let c = QueryCache::new();
        c.put(FetchedRecord {
            key: k(&["sessions"]),
            generation: 1,
            value: serde_json::json!({"x": 1}),
            stale_after: Duration::from_secs(5),
        });
        let g = c.peek(&k(&["sessions"]));
        assert!(g.value.is_some());
        assert!(!g.will_refetch);
    }

    #[test]
    fn missing_key_says_will_refetch() {
        let c = QueryCache::new();
        let g = c.peek(&k(&["sessions"]));
        assert!(g.value.is_none());
        assert!(g.will_refetch);
    }

    #[test]
    fn stale_value_triggers_refetch() {
        let c = QueryCache::new();
        c.put(FetchedRecord {
            key: k(&["sessions"]),
            generation: 1,
            value: serde_json::json!(0),
            stale_after: Duration::from_millis(0),
        });
        let g = c.peek(&k(&["sessions"]));
        assert!(g.value.is_some());
        assert!(g.will_refetch);
    }

    #[test]
    fn in_flight_dedupe_returns_same_generation() {
        let c = QueryCache::new();
        let t1 = c.begin_fetch(&k(&["sessions"]));
        let t2 = c.begin_fetch(&k(&["sessions"]));
        assert!(!t1.already_in_flight);
        assert!(t2.already_in_flight);
        assert_eq!(t1.generation, t2.generation);
    }

    #[test]
    fn older_generation_result_discarded() {
        let c = QueryCache::new();
        let _ = c.begin_fetch(&k(&["sessions"])); // generation 1
        let _ = c.invalidate(&["sessions"]); // bumps to 2
        // Late result from generation 1 is discarded:
        c.put(FetchedRecord {
            key: k(&["sessions"]),
            generation: 1,
            value: serde_json::json!("stale"),
            stale_after: Duration::from_secs(5),
        });
        let g = c.peek(&k(&["sessions"]));
        assert!(g.value.is_none());
    }

    #[test]
    fn prefix_invalidation_marks_stale() {
        let c = QueryCache::new();
        c.put(FetchedRecord {
            key: k(&["session-span-tree", "cid-1"]),
            generation: 1,
            value: serde_json::json!(0),
            stale_after: Duration::from_secs(60),
        });
        let matched = c.invalidate(&["session-span-tree"]);
        assert_eq!(matched.len(), 1);
        let g = c.peek(&k(&["session-span-tree", "cid-1"]));
        assert!(g.value.is_some()); // still returned (SWR)
        assert!(g.will_refetch);
    }

    #[test]
    fn lru_evicts_span_keys_over_cap() {
        let c = QueryCache::new();
        for i in 0..(SPAN_LRU_CAP + 50) {
            c.put(FetchedRecord {
                key: k(&["span", &format!("t{i}"), &format!("s{i}")]),
                generation: 1,
                value: serde_json::json!(i),
                stale_after: Duration::from_secs(60),
            });
        }
        assert!(c.len() <= SPAN_LRU_CAP);
        // The first 50 should have been evicted.
        let g0 = c.peek(&k(&["span", "t0", "s0"]));
        assert!(g0.value.is_none());
        // The latest is retained.
        let last = SPAN_LRU_CAP + 49;
        let gl = c.peek(&k(&["span", &format!("t{last}"), &format!("s{last}")]));
        assert!(gl.value.is_some());
    }

    #[test]
    fn ws_table_session_chat_turn_invalidates_sessions() {
        let prefixes = ws_invalidation_prefixes(WsKind::Derived, WsEntity::ChatTurn);
        let strs: Vec<Vec<&str>> = prefixes
            .iter()
            .map(|p| p.iter().copied().collect())
            .collect();
        assert!(strs.contains(&vec!["sessions"]));
        assert!(strs.contains(&vec!["session-contexts"]));
        assert!(strs.contains(&vec!["traces"])); // derived/* per table
    }

    #[test]
    fn ws_table_span_span_invalidates_spans_traces_raw() {
        let prefixes = ws_invalidation_prefixes(WsKind::Span, WsEntity::Span);
        let strs: Vec<Vec<&str>> = prefixes
            .iter()
            .map(|p| p.iter().copied().collect())
            .collect();
        for needle in [
            vec!["traces"],
            vec!["session-span-tree"],
            vec!["session-contexts"],
            vec!["spans"],
            vec!["raw"],
            vec!["traces-detail"],
        ] {
            assert!(strs.contains(&needle), "missing {needle:?} in {strs:?}");
        }
    }

    #[test]
    fn ws_table_trace_only_hits_trace_keys() {
        let prefixes = ws_invalidation_prefixes(WsKind::Trace, WsEntity::Trace);
        let strs: Vec<Vec<&str>> = prefixes
            .iter()
            .map(|p| p.iter().copied().collect())
            .collect();
        assert!(strs.contains(&vec!["traces"]));
        assert!(strs.contains(&vec!["session-span-tree"]));
        assert!(!strs.contains(&vec!["sessions"]));
    }

    #[tokio::test]
    async fn cache_get_returns_fresh_without_calling_fetcher() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = Arc::new(QueryCache::new());
        cache.put(FetchedRecord {
            key: k(&["sessions"]),
            generation: 1,
            value: serde_json::json!({"hit": true}),
            stale_after: Duration::from_secs(60),
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let v = cache_get(&cache, k(&["sessions"]), Duration::from_secs(60), || async move {
            c2.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"missed": true}))
        })
        .await
        .unwrap();
        assert_eq!(v, serde_json::json!({"hit": true}));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn cache_get_refetches_when_stale() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = Arc::new(QueryCache::new());
        cache.put(FetchedRecord {
            key: k(&["sessions"]),
            generation: 1,
            value: serde_json::json!({"old": true}),
            stale_after: Duration::from_millis(0),
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let v = cache_get(&cache, k(&["sessions"]), Duration::from_secs(60), || async move {
            c2.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({"new": true}))
        })
        .await
        .unwrap();
        assert_eq!(v, serde_json::json!({"new": true}));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cache_get_dedupes_concurrent_fetches() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = Arc::new(QueryCache::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let mk = || {
            let c2 = calls.clone();
            let cache = cache.clone();
            tokio::spawn(async move {
                cache_get(&cache, k(&["sessions"]), Duration::from_secs(60), || async move {
                    // Simulate slow fetch so concurrent calls coalesce.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    c2.fetch_add(1, Ordering::SeqCst);
                    Ok(serde_json::json!({"v": 1}))
                })
                .await
            })
        };
        let a = mk();
        let b = mk();
        let c = mk();
        let _ = a.await.unwrap().unwrap();
        let _ = b.await.unwrap().unwrap();
        let _ = c.await.unwrap().unwrap();
        // Only one underlying fetcher invocation.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn swr_read_returns_typed_cached_value_without_fetching_under_readonly() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Sample {
            n: i32,
        }
        let cache = Arc::new(QueryCache::new());
        cache.put(FetchedRecord {
            key: k(&["sessions"]),
            generation: 1,
            value: serde_json::to_value(Sample { n: 7 }).unwrap(),
            stale_after: Duration::from_secs(60),
        });
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let v: Option<Sample> = swr_read(
            &cache,
            k(&["sessions"]),
            Duration::from_secs(60),
            FetchPolicy::ReadOnly,
            move || async move {
                c2.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({"n": 99}))
            },
        );
        assert_eq!(v, Some(Sample { n: 7 }));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn swr_read_returns_none_without_fetch_when_missing_under_readonly() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = Arc::new(QueryCache::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let v: Option<serde_json::Value> = swr_read(
            &cache,
            k(&["missing"]),
            Duration::from_secs(60),
            FetchPolicy::ReadOnly,
            move || async move {
                c2.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!(null))
            },
        );
        assert!(v.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn swr_read_spawns_background_fetch_on_miss_under_swr() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = Arc::new(QueryCache::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let c2 = calls.clone();
        let v: Option<serde_json::Value> = swr_read(
            &cache,
            k(&["sessions"]),
            Duration::from_secs(60),
            FetchPolicy::Swr,
            move || async move {
                c2.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({"hit": true}))
            },
        );
        // First call returns None (cache miss) and dispatches the fetch.
        assert!(v.is_none());
        // Yield so the spawned task can finish on the current-thread runtime.
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(calls.load(Ordering::SeqCst), 1, "fetcher must have run");
        let v2: Option<serde_json::Value> = swr_read(
            &cache,
            k(&["sessions"]),
            Duration::from_secs(60),
            FetchPolicy::ReadOnly,
            || async move { unreachable!() },
        );
        assert_eq!(v2, Some(serde_json::json!({"hit": true})));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn swr_read_returns_none_on_deserialize_mismatch() {
        use std::sync::Arc;
        #[derive(serde::Deserialize)]
        struct Sample {
            #[allow(dead_code)]
            n: i32,
        }
        let cache = Arc::new(QueryCache::new());
        cache.put(FetchedRecord {
            key: k(&["sessions"]),
            generation: 1,
            value: serde_json::json!({"wrong": "shape"}),
            stale_after: Duration::from_secs(60),
        });
        let v: Option<Sample> = swr_read(
            &cache,
            k(&["sessions"]),
            Duration::from_secs(60),
            FetchPolicy::ReadOnly,
            || async move { unreachable!() },
        );
        assert!(v.is_none(), "shape mismatch must yield None");
    }
}
