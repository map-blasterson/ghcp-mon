//! Ingest paths:
//! - file-exporter JSON-lines (replay + tests)
//! - OTLP/HTTP (live)
//!
//! Both convert to the internal `Envelope` shape in `crate::model`.

use crate::db::dao::insert_raw;
use crate::error::{AppError, AppResult};
use crate::model::*;
use crate::normalize::{self, NormalizeCtx};
use crate::server::AppState;
use serde_json::{Map, Value};
use sqlx::SqlitePool;
use tracing::{info, warn};

pub mod otlp;
pub mod replay;

/// Ingest a single envelope: persist raw text, then normalize.
pub async fn ingest_envelope(state: &AppState, source: &str, raw_text: &str, env: Envelope) -> AppResult<i64> {
    let raw_id = insert_raw(&state.pool, source, env.type_tag(), Some("application/json"), raw_text).await?;
    let ctx = NormalizeCtx { pool: &state.pool, bus: &state.bus, raw_record_id: raw_id };
    if let Err(e) = normalize::handle_envelope(&ctx, &env).await {
        warn!("normalize failed: {e:?}");
    }
    info!(source, raw_id, kind=env.type_tag(), "ingested envelope");
    Ok(raw_id)
}

/// Parse one file-exporter JSON line (`{"type": "span"|"metric"|"log", ...}`).
pub fn parse_file_exporter_line(line: &str) -> AppResult<Envelope> {
    let env: Envelope = serde_json::from_str(line)
        .map_err(|e| AppError::BadRequest(format!("file-exporter parse: {e}")))?;
    Ok(env)
}

/// Run a JSON-lines file (the file-exporter format) through ingest.
/// Returns the number of envelopes ingested.
pub async fn ingest_jsonl_file(state: &AppState, path: &std::path::Path, source: &str) -> AppResult<usize> {
    use tokio::io::AsyncBufReadExt;
    let f = tokio::fs::File::open(path).await
        .map_err(|e| AppError::BadRequest(format!("open {}: {}", path.display(), e)))?;
    let reader = tokio::io::BufReader::new(f);
    let mut lines = reader.lines();
    let mut count = 0usize;
    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        match parse_file_exporter_line(trimmed) {
            Ok(env) => {
                ingest_envelope(state, source, trimmed, env).await?;
                count += 1;
            }
            Err(e) => warn!("skip line: {e}"),
        }
    }
    Ok(count)
}

// ---------------------- OTLP/JSON conversion --------------------------------

/// Flatten OTLP `attributes` (array of `{key, value: AnyValue}`) into a JSON
/// object using simple unwrapping: stringValue/intValue/doubleValue/boolValue/arrayValue.
pub fn flatten_otlp_attributes(arr: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    if let Some(items) = arr.as_array() {
        for kv in items {
            let key = kv.get("key").and_then(|v| v.as_str()).map(String::from);
            let val = kv.get("value").map(unwrap_any_value).unwrap_or(Value::Null);
            if let Some(k) = key { out.insert(k, val); }
        }
    }
    out
}

fn unwrap_any_value(v: &Value) -> Value {
    if let Some(s) = v.get("stringValue") { return s.clone(); }
    if let Some(s) = v.get("intValue") {
        // OTLP/JSON encodes int64 as string by spec
        if let Some(t) = s.as_str() {
            if let Ok(i) = t.parse::<i64>() { return Value::from(i); }
        }
        return s.clone();
    }
    if let Some(s) = v.get("doubleValue") { return s.clone(); }
    if let Some(s) = v.get("boolValue") { return s.clone(); }
    if let Some(s) = v.get("bytesValue") { return s.clone(); }
    if let Some(arr) = v.get("arrayValue").and_then(|a| a.get("values")) {
        if let Some(items) = arr.as_array() {
            return Value::Array(items.iter().map(unwrap_any_value).collect());
        }
    }
    if let Some(kvl) = v.get("kvlistValue").and_then(|a| a.get("values")) {
        return Value::Object(flatten_otlp_attributes(kvl));
    }
    v.clone()
}

fn parse_unix_nano(v: &Value) -> Option<i64> {
    match v {
        Value::String(s) => s.parse::<i64>().ok(),
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        _ => None,
    }
}

/// Convert a single OTLP/JSON traces request body into a list of `SpanEnvelope`.
pub fn otlp_traces_to_envelopes(body: &Value) -> Vec<SpanEnvelope> {
    let mut out = Vec::new();
    let resource_spans = body.get("resourceSpans").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for rs in &resource_spans {
        let resource = rs.get("resource").map(|r| Resource {
            attributes: flatten_otlp_attributes(r.get("attributes").unwrap_or(&Value::Null)),
            schema_url: rs.get("schemaUrl").and_then(|v| v.as_str()).map(String::from),
        });
        if let Some(scope_spans) = rs.get("scopeSpans").and_then(|v| v.as_array()) {
            for ss in scope_spans {
                let scope = ss.get("scope").map(|s| InstrumentationScope {
                    name: s.get("name").and_then(|v| v.as_str()).map(String::from),
                    version: s.get("version").and_then(|v| v.as_str()).map(String::from),
                });
                if let Some(spans) = ss.get("spans").and_then(|v| v.as_array()) {
                    for sp in spans {
                        let trace_id = sp.get("traceId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let span_id = sp.get("spanId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let parent = sp.get("parentSpanId").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(String::from);
                        let name = sp.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let kind = sp.get("kind").and_then(|v| v.as_i64());
                        let start = sp.get("startTimeUnixNano").and_then(parse_unix_nano).unwrap_or(0);
                        let end = sp.get("endTimeUnixNano").and_then(parse_unix_nano);
                        let attrs = sp.get("attributes").map(flatten_otlp_attributes).unwrap_or_default();
                        let mut events = Vec::new();
                        if let Some(evs) = sp.get("events").and_then(|v| v.as_array()) {
                            for ev in evs {
                                let evt_attrs = ev.get("attributes").map(flatten_otlp_attributes).unwrap_or_default();
                                let t = ev.get("timeUnixNano").and_then(parse_unix_nano).unwrap_or(0);
                                events.push(EventEnvelope {
                                    name: ev.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                    time: HrTime::Nanos(t),
                                    attributes: evt_attrs,
                                });
                            }
                        }
                        let status = sp.get("status").map(|s| SpanStatus {
                            code: s.get("code").and_then(|v| v.as_i64()).unwrap_or(0),
                            message: s.get("message").and_then(|v| v.as_str()).map(String::from),
                        });
                        out.push(SpanEnvelope {
                            kind_tag: "span".into(),
                            trace_id, span_id, parent_span_id: parent, name, kind,
                            start_time: HrTime::Nanos(start),
                            end_time: end.map(HrTime::Nanos),
                            attributes: attrs,
                            events,
                            status,
                            resource: resource.clone(),
                            instrumentation_scope: scope.clone(),
                        });
                    }
                }
            }
        }
    }
    out
}

/// Convert an OTLP/JSON metrics request body into MetricEnvelopes.
pub fn otlp_metrics_to_envelopes(body: &Value) -> Vec<MetricEnvelope> {
    let mut out = Vec::new();
    let resource_metrics = body.get("resourceMetrics").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for rm in &resource_metrics {
        let resource = rm.get("resource").map(|r| Resource {
            attributes: flatten_otlp_attributes(r.get("attributes").unwrap_or(&Value::Null)),
            schema_url: rm.get("schemaUrl").and_then(|v| v.as_str()).map(String::from),
        });
        if let Some(scope_metrics) = rm.get("scopeMetrics").and_then(|v| v.as_array()) {
            for sm in scope_metrics {
                let scope = sm.get("scope").map(|s| InstrumentationScope {
                    name: s.get("name").and_then(|v| v.as_str()).map(String::from),
                    version: s.get("version").and_then(|v| v.as_str()).map(String::from),
                });
                if let Some(metrics) = sm.get("metrics").and_then(|v| v.as_array()) {
                    for m in metrics {
                        let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let description = m.get("description").and_then(|v| v.as_str()).map(String::from);
                        let unit = m.get("unit").and_then(|v| v.as_str()).map(String::from);
                        let data_points = collect_otlp_metric_points(m);
                        out.push(MetricEnvelope {
                            kind_tag: "metric".into(),
                            name, description, unit, data_points,
                            resource: resource.clone(),
                            instrumentation_scope: scope.clone(),
                        });
                    }
                }
            }
        }
    }
    out
}

fn collect_otlp_metric_points(m: &Value) -> Vec<MetricDataPoint> {
    let mut out = Vec::new();
    for kind in ["gauge", "sum", "histogram", "exponentialHistogram", "summary"] {
        if let Some(node) = m.get(kind) {
            if let Some(dps) = node.get("dataPoints").and_then(|v| v.as_array()) {
                for dp in dps {
                    let attrs = dp.get("attributes").map(flatten_otlp_attributes).unwrap_or_default();
                    let start = dp.get("startTimeUnixNano").and_then(parse_unix_nano);
                    let end = dp.get("timeUnixNano").and_then(parse_unix_nano);
                    out.push(MetricDataPoint {
                        attributes: attrs,
                        start_time: start.map(HrTime::Nanos),
                        end_time: end.map(HrTime::Nanos),
                        value: dp.clone(),
                    });
                }
            }
        }
    }
    out
}

/// Persist raw OTLP body as a single `raw_records` row (so the request is
/// preserved verbatim) and return its id. Each derived envelope gets its own
/// `raw_records` row inside `ingest_envelope` for traceability.
pub async fn persist_raw_request(pool: &SqlitePool, source: &str, content_type: Option<&str>, record_type: &str, body: &str) -> sqlx::Result<i64> {
    insert_raw(pool, source, record_type, content_type, body).await
}
