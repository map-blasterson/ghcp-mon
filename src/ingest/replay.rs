use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use crate::server::AppState;
use crate::error::AppResult;
use crate::ingest::ingest_jsonl_file;

#[derive(Deserialize)]
pub struct ReplayReq {
    pub path: String,
}

pub async fn replay(State(state): State<AppState>, Json(req): Json<ReplayReq>) -> AppResult<impl IntoResponse> {
    let path = std::path::PathBuf::from(&req.path);
    let count = ingest_jsonl_file(&state, &path, "replay").await?;
    Ok(Json(json!({"path": req.path, "ingested": count})))
}
