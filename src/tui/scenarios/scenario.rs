//! `Scenario` trait + render/key/WS context.
//!
//! ## Design (rationale in `frontend/tui/llr/` once filed)
//!
//! Per-column behaviour lives behind a `Scenario` trait so the top-level
//! [`crate::tui::app::App`] reduces to event-loop wiring + a dispatch
//! map of `Box<dyn Scenario>` keyed by column id. The previous
//! arrangement — per-scenario `state: HashMap<String, _>` and a pile of
//! adapter `draw_*` / `*_key` methods on `App` — forced the borrow
//! checker to be appeased by snapshotting per-column data into a 7-tuple
//! `Vec` every frame and made every cross-cutting walk (follow-mode,
//! search propagation, hover publishing) reach back into App internals.
//!
//! ### Invariants
//!
//! * Scenarios MUST NOT mutate `Workspace`, the confirm modal, the
//!   `hovered_chat_pk` pubsub, or trigger persistence. They return
//!   [`ScenarioEffect`]s; the App applies them after the per-scenario
//!   borrow ends. This keeps the borrow graph small (`&mut Scenario` +
//!   `&mut Ctx` are disjoint borrows of App fields) and concentrates
//!   cross-column reasoning in one place.
//! * [`Ctx`] is constructed inside the dispatch branch, not around the
//!   whole loop, so legacy code paths (Spans, pending migration) can
//!   continue to take `&mut self` on the same iteration.
//! * Key handlers MUST NOT trigger network fetches. The cache fetcher
//!   lifecycle is owned by render-path `swr_read` and by explicit
//!   debounced kickers like [`Ctx::kick_search_debounce`]. This is
//!   discipline-by-convention today; a future `DrawCtx`/`KeyCtx` split
//!   can encode it in the type system.
//! * `on_ws_batch` runs once per scenario per WS batch (post-
//!   invalidation, pre-draw). Effects from all scenarios are collected
//!   in column order and applied after the loop, so an earlier
//!   scenario's effect can never affect a later scenario's view of
//!   workspace state in the same batch.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use serde_json::Value;

use crate::tui::api::ApiClient;
use crate::tui::cache::{
    FetchPolicy, QueryCache, cache_get, qkey, swr_read,
};
use crate::tui::model::{
    ContextSnapshot, ListSessionContextsResponse, ListSessionsResponse, ListTracesResponse,
    SearchResponse, SessionSpanTreeResponse, SessionSummary, SpanDetail, SpanNode, TraceSummary,
};
use crate::tui::workspace::{ColumnConfig, Workspace};

use super::ScenarioEffect;

/// Outcome of a key dispatch. Carries whether the scenario consumed the
/// key and any cross-cutting effects to apply afterwards.
#[derive(Debug, Default)]
pub struct KeyOutcome {
    pub consumed: bool,
    pub effects: Vec<ScenarioEffect>,
}

impl KeyOutcome {
    pub fn pass() -> Self {
        Self { consumed: false, effects: Vec::new() }
    }
    pub fn consumed() -> Self {
        Self { consumed: true, effects: Vec::new() }
    }
    pub fn with_effects(mut self, effects: Vec<ScenarioEffect>) -> Self {
        self.effects = effects;
        self
    }
}

/// Metadata about a WS batch that scenarios can gate their hooks on. Kept
/// minimal — add fields as the migration of Spans (follow-mode advance)
/// surfaces real needs.
#[derive(Debug, Clone, Copy, Default)]
pub struct WsBatchMeta {
    /// True when the batch contained at least one envelope whose
    /// `(kind, entity)` invalidates span-tree-shaped data.
    pub touches_spans: bool,
}

/// Output of `Scenario::draw` that the renderer needs to surface upward
/// (e.g. spinner-on-screen for the event loop's anim deadline). Re-exported
/// from `app` for ergonomics so scenarios don't need to know about App.
pub use crate::tui::app::DrawOutcome;

/// Per-call context handed to a scenario's `draw` / `handle_key` /
/// `on_ws_batch`. Holds disjoint borrows of [`crate::tui::app::App`] fields
/// so the scenario can read cached data, peek workspace state, and
/// schedule background fetches without ever touching App directly.
///
/// **Do not** keep a `Ctx` alive across multiple scenario calls — it must
/// be reconstructed inside each per-column dispatch so legacy code paths
/// (Spans pending migration, top-level draw scaffolding) keep their
/// `&mut self` access.
pub struct Ctx<'a> {
    api: &'a ApiClient,
    cache: &'a Arc<QueryCache>,
    workspace: &'a Workspace,
    hovered_chat_pk: &'a Arc<RwLock<Option<i64>>>,
    span_detail_memo: &'a mut HashMap<(String, String), (u64, Rc<SpanDetail>)>,
}

impl<'a> Ctx<'a> {
    /// Construct a fresh context. Borrows are disjoint App fields — the
    /// caller is responsible for invoking this only inside a dispatch
    /// branch (see invariants above).
    pub fn new(
        api: &'a ApiClient,
        cache: &'a Arc<QueryCache>,
        workspace: &'a Workspace,
        hovered_chat_pk: &'a Arc<RwLock<Option<i64>>>,
        span_detail_memo: &'a mut HashMap<(String, String), (u64, Rc<SpanDetail>)>,
    ) -> Self {
        Self { api, cache, workspace, hovered_chat_pk, span_detail_memo }
    }

    /// Read-only view of the workspace (column list + per-column config).
    /// Mutations go via [`ScenarioEffect`].
    pub fn workspace(&self) -> &Workspace {
        self.workspace
    }

    /// Snapshot of the cross-column hovered chat span pk (publisher:
    /// Spans; consumer: Context Growth Widget). Writes go via
    /// [`ScenarioEffect::SetHoveredChatPk`] once that variant lands.
    pub fn hovered_chat_pk(&self) -> Option<i64> {
        self.hovered_chat_pk.read().ok().and_then(|g| *g)
    }

    // ---- semantic cache helpers ----

    pub fn cached_sessions(&self) -> Vec<SessionSummary> {
        let api = self.api.clone();
        swr_read::<ListSessionsResponse, _, _>(
            self.cache,
            qkey(["sessions"]),
            std::time::Duration::from_secs(5),
            FetchPolicy::Swr,
            move || async move {
                let r = api.list_sessions(Some(50), None).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )
        .map(|r| r.sessions)
        .unwrap_or_default()
    }

    pub fn cached_traces(&self) -> Vec<TraceSummary> {
        let api = self.api.clone();
        swr_read::<ListTracesResponse, _, _>(
            self.cache,
            qkey(["traces"]),
            std::time::Duration::from_secs(5),
            FetchPolicy::Swr,
            move || async move {
                let r = api.list_traces(Some(50), None).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )
        .map(|r| r.traces)
        .unwrap_or_default()
    }

    /// Read-only counterpart of [`Self::cached_traces`]. Returns
    /// whatever is currently cached without dispatching a background
    /// fetch. Used by key handlers (per the "key handlers MUST NOT
    /// trigger fetches" invariant) — render paths should use
    /// [`Self::cached_traces`] which is SWR.
    pub fn cached_traces_readonly(&self) -> Vec<TraceSummary> {
        swr_read::<ListTracesResponse, _, _>(
            self.cache,
            qkey(["traces"]),
            std::time::Duration::from_secs(5),
            FetchPolicy::ReadOnly,
            || async move { unreachable!("ReadOnly policy never invokes the fetcher") },
        )
        .map(|r| r.traces)
        .unwrap_or_default()
    }

    /// Read-or-fetch `["session-span-tree", cid]`. Returns the COMPLETE
    /// server tree (per the cache contract — DELTA's prior-chat-span walk
    /// and the file-touches walk MUST read this, not Spans'
    /// reveal-filtered view).
    pub fn cached_session_span_tree(&self, cid: &str) -> Vec<SpanNode> {
        let api = self.api.clone();
        let cid_s = cid.to_string();
        swr_read::<SessionSpanTreeResponse, _, _>(
            self.cache,
            qkey(["session-span-tree", cid]),
            std::time::Duration::from_secs(5),
            FetchPolicy::Swr,
            move || async move {
                let r = api.get_session_span_tree(&cid_s).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )
        .map(|r| r.tree)
        .unwrap_or_default()
    }

    /// True iff the cache currently holds *any* value for the session's
    /// span tree (used to distinguish "loading" from "no touches").
    pub fn session_span_tree_loaded(&self, cid: &str) -> bool {
        self.cache.peek(&qkey(["session-span-tree", cid])).value.is_some()
    }

    pub fn cached_session_contexts(&self, cid: &str) -> Vec<ContextSnapshot> {
        let api = self.api.clone();
        let cid_s = cid.to_string();
        swr_read::<ListSessionContextsResponse, _, _>(
            self.cache,
            qkey(["session-contexts", cid]),
            std::time::Duration::from_secs(5),
            FetchPolicy::Swr,
            move || async move {
                let r = api.list_session_contexts(&cid_s).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )
        .map(|r| r.context_snapshots)
        .unwrap_or_default()
    }

    /// Read the `SpanDetail` for `(trace_id, span_id)` through the query
    /// cache, memoising the deserialized value keyed by the cache entry's
    /// `generation`. Re-deserializes only when the cache entry changes
    /// (background fetch completion or WS invalidation). Survives across
    /// draws — the memo lives on `App`, lent through this `Ctx`.
    pub fn cached_span_detail(
        &mut self,
        trace_id: &str,
        span_id: &str,
    ) -> Option<Rc<SpanDetail>> {
        let memo_key = (trace_id.to_string(), span_id.to_string());
        let cache_key = qkey(["span", trace_id, span_id]);

        let current_gen = self
            .cache
            .peek(&cache_key)
            .value
            .as_ref()
            .map(|c| c.generation);
        if let (Some(g), Some((memo_g, rc))) =
            (current_gen, self.span_detail_memo.get(&memo_key))
        {
            if *memo_g == g {
                return Some(rc.clone());
            }
        }

        let api = self.api.clone();
        let tid = trace_id.to_string();
        let sid = span_id.to_string();
        let parsed = swr_read::<SpanDetail, _, _>(
            self.cache,
            cache_key.clone(),
            std::time::Duration::from_secs(30),
            FetchPolicy::Swr,
            move || async move {
                let r = api.get_span(&tid, &sid).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )?;
        let rc = Rc::new(parsed);
        let stored_gen = self
            .cache
            .peek(&cache_key)
            .value
            .as_ref()
            .map(|c| c.generation)
            .unwrap_or(0);
        self.span_detail_memo.insert(memo_key, (stored_gen, rc.clone()));

        // Coarse 2×LRU bound prevents unbounded growth on long sessions.
        if self.span_detail_memo.len() > 2 * crate::tui::cache::SPAN_LRU_CAP {
            self.span_detail_memo.clear();
        }
        Some(rc)
    }

    /// Read-only lookup of `["search-spans", session, q]`. The fetch
    /// lifecycle is owned by [`Self::kick_search_debounce`] (per the
    /// `TUI Spans search input edit semantics` LLR's 300 ms debounce
    /// contract) — render MUST NOT kick its own fetch here.
    pub fn cached_search_hits(&self, session: &str, q: &str) -> Option<SearchResponse> {
        if q.is_empty() {
            return None;
        }
        swr_read::<SearchResponse, _, _>(
            self.cache,
            qkey(["search-spans", session, q]),
            std::time::Duration::from_secs(5),
            FetchPolicy::ReadOnly,
            || async move { unreachable!("ReadOnly policy never invokes the fetcher") },
        )
    }

    /// 300 ms debounce kick for the server-side span search. Spawns a
    /// background fetch; render reads through [`Self::cached_search_hits`]
    /// with `FetchPolicy::ReadOnly`.
    pub fn kick_search_debounce(&self, session: &str, q: String) {
        if q.is_empty() {
            return;
        }
        let cache = self.cache.clone();
        let api = self.api.clone();
        let session = session.to_string();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let _ = cache_get(
                &cache,
                qkey(["search-spans", &session, &q]),
                std::time::Duration::from_secs(5),
                || async move {
                    let r = api.search_spans(&q, &session, Some(200)).await?;
                    Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
                },
            )
            .await;
        });
    }
}

/// One column's worth of scenario behaviour. Implementors own their
/// scenario state. App holds `HashMap<String, Box<dyn Scenario>>` keyed
/// by column id and dispatches to the matching scenario from
/// `draw_workspace` / `scenario_handle_key` / `on_ws_envelopes`.
pub trait Scenario: std::any::Any {
    /// Downcast escape hatch — used by App for the small set of
    /// behaviours that need typed access to a specific scenario
    /// (currently `widget_select_current` reaching into `SpansScenario`
    /// to reposition row cursors). Implementors return `self`.
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Render this column. Scenarios MUST flip
    /// `outcome.spinner_visible = true` whenever they render an animated
    /// affordance (rolling dots, progress bar, etc.) so the event loop
    /// can schedule the next animation wake.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        col_id: &str,
        config: &ColumnConfig,
        area: Rect,
        buf: &mut Buffer,
        focused: bool,
        outcome: &mut DrawOutcome,
    );

    /// Handle a key while this column is focused. Returns whether the
    /// key was consumed plus any [`ScenarioEffect`]s to apply.
    fn handle_key(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        col_id: &str,
        config: &ColumnConfig,
        k: KeyEvent,
    ) -> KeyOutcome;

    /// Per-scenario keymap entries for the help overlay. Some scenarios
    /// have mode-dependent keymaps (search input vs default) and read
    /// `config` to render the right set.
    fn keymap_entries(&self, config: &ColumnConfig) -> Vec<(String, String)>;

    /// Called once per WS batch (after cache invalidation, before draw).
    /// Default: no-op. Returns effects to apply post-batch so cross-
    /// scenario ordering is deterministic.
    fn on_ws_batch(
        &mut self,
        _ctx: &mut Ctx<'_>,
        _col_idx: usize,
        _col_id: &str,
        _config: &ColumnConfig,
        _meta: &WsBatchMeta,
    ) -> Vec<ScenarioEffect> {
        Vec::new()
    }

    /// Called once per scenario after the cache emits a "value changed"
    /// notification (e.g. a background SWR fetch completed). Used by
    /// follow-mode to re-run the latest-tool-span walk once the freshly
    /// arrived `["session-span-tree", cid]` is queryable. Default no-op.
    fn on_cache_changed(
        &mut self,
        _ctx: &mut Ctx<'_>,
        _col_idx: usize,
        _col_id: &str,
        _config: &ColumnConfig,
    ) -> Vec<ScenarioEffect> {
        Vec::new()
    }

    /// Called from the event loop's animation tick. Drain due
    /// reveal-queue entries or advance other time-based state. Default
    /// no-op. Effects are applied post-batch in column order.
    fn tick(
        &mut self,
        _ctx: &mut Ctx<'_>,
        _col_idx: usize,
        _col_id: &str,
        _config: &ColumnConfig,
        _now_ms: u64,
    ) -> Vec<ScenarioEffect> {
        Vec::new()
    }

    /// Earliest wall-clock ms at which the scenario must be re-ticked
    /// (e.g. the head of its reveal queue). `None` when nothing is
    /// scheduled. Aggregated by `App::next_anim_deadline_ms`.
    fn next_anim_deadline(&self, _config: &ColumnConfig) -> Option<u64> {
        None
    }

    /// True when the scenario is in a text-input mode that should
    /// intercept printable keys before any other dispatch layer (App
    /// Layer 1). The scenario's `handle_key` is then invoked with the
    /// key; if it returns `consumed = true`, dispatch stops, otherwise
    /// the key falls through to subsequent layers (modal/widget/column/
    /// global). Default false.
    fn text_input_active(&self, _config: &ColumnConfig) -> bool {
        false
    }

    /// True when the scenario is showing a column-scoped modal popover
    /// that should consume all keys (App Layer 2). When true, App
    /// dispatches the key via `handle_key` and ALWAYS treats it as
    /// consumed, regardless of the scenario's return value, so non-
    /// matching keys cannot fall through (e.g. `q` mustn't quit while
    /// a session picker is open). Default false.
    fn popover_active(&self, _config: &ColumnConfig) -> bool {
        false
    }
}
