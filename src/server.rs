use axum::{routing::{get, post}, Router};
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::ws::{Broadcaster, handler::ws_handler};
use crate::ingest::otlp;
use crate::api;
use crate::static_assets::static_handler;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub bus: Broadcaster,
    /// Explicit `--session-state-dir` override. When `Some`, takes precedence
    /// over `$COPILOT_SESSION_STATE_DIR` and `$HOME/.copilot/session-state`.
    pub session_state_dir_override: Arc<Option<PathBuf>>,
}

/// OTLP receiver router (mounted on the OTLP listener).
pub fn otlp_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/traces", post(otlp::traces))
        .route("/v1/metrics", post(otlp::metrics))
        .route("/v1/logs", post(otlp::logs))
        .layer(TraceLayer::new_for_http())
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
        .with_state(state)
}

/// API + WebSocket router (mounted on the API listener).
pub fn api_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/healthz", get(api::healthz))
        .route("/api/sessions", get(api::list_sessions))
        .route("/api/sessions/:cid", get(api::get_session).delete(api::delete_session))
        .route("/api/sessions/:cid/span-tree", get(api::get_session_span_tree))
        .route("/api/sessions/:cid/contexts", get(api::list_session_contexts))
        .route("/api/spans", get(api::list_spans))
        .route("/api/spans/:trace_id/:span_id", get(api::get_span))
        .route("/api/traces", get(api::list_traces))
        .route("/api/traces/:trace_id", get(api::get_trace))
        .route("/api/raw", get(api::list_raw))
        .route("/api/replay", post(crate::ingest::replay::replay))
        .route("/ws/events", get(ws_handler))
        .fallback(static_handler)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024))
        .with_state(state)
}

pub async fn serve(state: AppState, otlp_addr: SocketAddr, api_addr: SocketAddr) -> anyhow::Result<()> {
    let otlp_app = otlp_router(state.clone());
    let api_app = api_router(state.clone());

    let otlp_listener = tokio::net::TcpListener::bind(otlp_addr).await?;
    let api_listener = tokio::net::TcpListener::bind(api_addr).await?;
    info!("OTLP listening on http://{otlp_addr}");
    info!("API+WS listening on http://{api_addr}");

    let otlp_h = tokio::spawn(async move { axum::serve(otlp_listener, otlp_app).await });
    let api_h = tokio::spawn(async move { axum::serve(api_listener, api_app).await });
    let _ = tokio::try_join!(flatten(otlp_h), flatten(api_h))?;
    Ok(())
}

async fn flatten(h: tokio::task::JoinHandle<std::io::Result<()>>) -> anyhow::Result<()> {
    h.await??;
    Ok(())
}
