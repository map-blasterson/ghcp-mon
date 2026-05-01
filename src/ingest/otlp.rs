use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use serde_json::{json, Value};
use crate::server::AppState;
use crate::error::{AppError, AppResult};
use crate::model::Envelope;
use crate::ingest::{ingest_envelope, otlp_traces_to_envelopes, otlp_metrics_to_envelopes};

const JSON_CT: &str = "application/json";
const PROTOBUF_CT: &str = "application/x-protobuf";

fn content_type(headers: &HeaderMap) -> String {
    headers.get("content-type").and_then(|v| v.to_str().ok()).unwrap_or(JSON_CT).to_lowercase()
}

pub async fn traces(State(state): State<AppState>, headers: HeaderMap, body: bytes::Bytes) -> AppResult<impl IntoResponse> {
    let ct = content_type(&headers);
    if ct.contains(PROTOBUF_CT) {
        return Err(AppError::NotImplemented(
            "OTLP/HTTP protobuf ingestion is not implemented yet; send application/json".into(),
        ));
    }
    let text = std::str::from_utf8(&body).map_err(|e| AppError::BadRequest(format!("utf8: {e}")))?;
    let value: Value = serde_json::from_str(text).map_err(|e| AppError::BadRequest(format!("json: {e}")))?;
    // Persist the raw request body as a single record, then derive envelopes.
    let _outer = crate::ingest::persist_raw_request(&state.pool, "otlp-http-json", Some(JSON_CT), "otlp-traces", text).await?;
    let envelopes = otlp_traces_to_envelopes(&value);
    let mut accepted = 0usize;
    for sp in envelopes {
        let env_text = serde_json::to_string(&sp)?;
        ingest_envelope(&state, "otlp-http-json", &env_text, Envelope::Span(Box::new(sp))).await?;
        accepted += 1;
    }
    Ok(Json(json!({"partialSuccess": {}, "accepted": accepted})))
}

pub async fn metrics(State(state): State<AppState>, headers: HeaderMap, body: bytes::Bytes) -> AppResult<impl IntoResponse> {
    let ct = content_type(&headers);
    if ct.contains(PROTOBUF_CT) {
        return Err(AppError::NotImplemented("OTLP/HTTP protobuf metrics ingestion is not implemented yet".into()));
    }
    let text = std::str::from_utf8(&body).map_err(|e| AppError::BadRequest(format!("utf8: {e}")))?;
    let value: Value = serde_json::from_str(text).map_err(|e| AppError::BadRequest(format!("json: {e}")))?;
    let _outer = crate::ingest::persist_raw_request(&state.pool, "otlp-http-json", Some(JSON_CT), "otlp-metrics", text).await?;
    let envelopes = otlp_metrics_to_envelopes(&value);
    let mut accepted = 0usize;
    for m in envelopes {
        let env_text = serde_json::to_string(&m)?;
        ingest_envelope(&state, "otlp-http-json", &env_text, Envelope::Metric(Box::new(m))).await?;
        accepted += 1;
    }
    Ok(Json(json!({"partialSuccess": {}, "accepted": accepted})))
}

pub async fn logs(State(state): State<AppState>, headers: HeaderMap, body: bytes::Bytes) -> AppResult<impl IntoResponse> {
    let ct = content_type(&headers);
    if ct.contains(PROTOBUF_CT) {
        return Err(AppError::NotImplemented("OTLP/HTTP protobuf logs ingestion is not implemented yet".into()));
    }
    let text = std::str::from_utf8(&body).map_err(|e| AppError::BadRequest(format!("utf8: {e}")))?;
    // Logs are persisted as raw only for now.
    let _outer = crate::ingest::persist_raw_request(&state.pool, "otlp-http-json", Some(JSON_CT), "otlp-logs", text).await?;
    Ok(Json(json!({"partialSuccess": {}, "accepted": 0})))
}
