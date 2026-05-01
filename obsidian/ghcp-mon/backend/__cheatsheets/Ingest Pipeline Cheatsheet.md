---
type: cheatsheet
---
Source: `src/ingest/mod.rs`. Crate path: `ghcp_mon::ingest`.

The pipeline core: persists raw JSON, normalizes envelopes, parses file-exporter lines, walks JSONL files, and provides OTLP-JSON conversion helpers used by [[OTLP Handlers Cheatsheet]].

Depends on: [[Model Envelope Cheatsheet]] (`Envelope`, `SpanEnvelope`, `MetricEnvelope`, `LogEnvelope`, `HrTime`, `Resource`, `InstrumentationScope`, `EventEnvelope`, `MetricDataPoint`, `SpanStatus`), [[DAO Cheatsheet]] (`insert_raw`), [[AppError Cheatsheet]], [[Server Router Cheatsheet]] (`AppState`), and `crate::normalize::{handle_envelope, NormalizeCtx}`.

## Extract

```rust
use crate::error::{AppError, AppResult};
use crate::model::*;
use crate::server::AppState;
use serde_json::{Map, Value};
use sqlx::SqlitePool;

pub mod otlp;
pub mod replay;

/// Persist raw text → call normalize::handle_envelope. Returns the new raw_records.id.
pub async fn ingest_envelope(
    state: &AppState, source: &str, raw_text: &str, env: Envelope,
) -> AppResult<i64>;

/// `serde_json::from_str` into `Envelope`, mapping parse errors to AppError::BadRequest.
pub fn parse_file_exporter_line(line: &str) -> AppResult<Envelope>;

/// Stream a JSON-lines file through ingest_envelope. Returns count of envelopes successfully ingested.
/// Blank lines are skipped silently; unparseable lines log a warn and are skipped (not counted).
pub async fn ingest_jsonl_file(
    state: &AppState, path: &std::path::Path, source: &str,
) -> AppResult<usize>;

/// OTLP/JSON attribute array → flat JSON object.
pub fn flatten_otlp_attributes(arr: &Value) -> Map<String, Value>;

/// OTLP/JSON traces request body → SpanEnvelope vector.
pub fn otlp_traces_to_envelopes(body: &Value) -> Vec<SpanEnvelope>;

/// OTLP/JSON metrics request body → MetricEnvelope vector.
pub fn otlp_metrics_to_envelopes(body: &Value) -> Vec<MetricEnvelope>;

/// Insert one raw_records row preserving the raw OTLP request body verbatim.
pub async fn persist_raw_request(
    pool: &SqlitePool, source: &str, content_type: Option<&str>,
    record_type: &str, body: &str,
) -> sqlx::Result<i64>;
```

Behavior notes (from public API surface):
- `ingest_envelope` writes one row to `raw_records` (`record_type = env.type_tag()`, `content_type = "application/json"`), then calls `normalize::handle_envelope`. Normalize errors are logged but **do not fail** the function — the function still returns `Ok(raw_id)` if the raw insert succeeded.
- `ingest_jsonl_file` uses `tokio::io::AsyncBufReadExt::lines`. Blank-after-trim lines are skipped without warning. Lines that fail `parse_file_exporter_line` are skipped with a `warn!`; only successfully ingested envelopes increment the counter.
- `parse_file_exporter_line` requires JSON with `"type"` set to `"span"`, `"metric"`, or `"log"` (see Envelope's `#[serde(tag = "type", rename_all = "lowercase")]`).
- `flatten_otlp_attributes` walks an array of `{key, value}` objects, where `value` is an OTLP `AnyValue` shape (`stringValue`, `intValue` (string-encoded int64 → parsed to `i64`), `doubleValue`, `boolValue`, `bytesValue`, `arrayValue.values`, `kvlistValue.values`). Unknown shapes pass through as `value.clone()`.
- `intValue` strings that fail to parse fall back to the original `Value` (string).
- `otlp_traces_to_envelopes` reads `body.resourceSpans[].scopeSpans[].spans[]`. Per span: `traceId`, `spanId`, `parentSpanId` (only kept when non-empty), `name`, `kind`, `startTimeUnixNano` / `endTimeUnixNano` (each parsed from string-or-number → `i64`), `attributes`, `events[]`, `status`. Resulting `SpanEnvelope.kind_tag = "span"`.
- `otlp_metrics_to_envelopes` walks `resourceMetrics[].scopeMetrics[].metrics[]` and collects data points across all known kinds: `gauge`, `sum`, `histogram`, `exponentialHistogram`, `summary`. Each `MetricDataPoint.value` is the **whole `dp` JSON node** (clone of the data-point object), not just the typed value.
- `persist_raw_request` is a one-line wrapper over [[DAO Cheatsheet]]'s `insert_raw`.

## Suggested Test Strategy

Pure helpers (no DB):
- `parse_file_exporter_line`: feed valid JSON for each of `span`/`metric`/`log` and assert variant via `matches!(env, Envelope::Span(_))`. Feed `"not json"` and assert `Err(AppError::BadRequest(_))`.
- `flatten_otlp_attributes`:
  - `stringValue` round-trip.
  - `intValue` with a string-encoded int (`"42"`) → `Value::from(42_i64)` (`v.is_i64()`).
  - `intValue` with a non-numeric string → returns the original `Value::String("...")` (the raw node).
  - `boolValue`, `doubleValue`, `bytesValue` pass-through.
  - `arrayValue.values` → recurses, returns `Value::Array`.
  - `kvlistValue.values` → recurses, returns nested `Value::Object`.
  - Empty/missing array → empty `Map`.
- `otlp_traces_to_envelopes`:
  - Build a minimal `serde_json::json!({"resourceSpans":[{"scopeSpans":[{"spans":[ ... ]}]}]})` and verify per-span field translation. Confirm empty `parentSpanId` is dropped to `None`. Confirm `startTimeUnixNano` accepts both string and number forms.
  - Confirm `kind_tag == "span"` on each output.
- `otlp_metrics_to_envelopes`: emit a body with one of each metric kind (e.g. a `gauge` with one `dataPoints`) and assert the resulting `MetricDataPoint.value` is the cloned data-point JSON (not just the numeric value).

DB-touching helpers — use `#[tokio::test]` + `ghcp_mon::db::open(&tempfile)` to obtain a real pool (see [[DB Module Cheatsheet]]):
- `persist_raw_request`: insert and `SELECT * FROM raw_records WHERE id = ?`. Verify all bound fields round-trip.
- `ingest_envelope`: build an `AppState { pool, bus: Broadcaster::new(64), session_state_dir_override: Arc::new(None) }`. Pass a small `Envelope::Span(...)`. Assert the returned `raw_id` matches a row in `raw_records` whose `record_type == "span"` and `body == raw_text` (verbatim) and `content_type == Some("application/json")`. Also assert that the `bus` produces span/trace events (subscribe before calling — see [[Broadcaster Cheatsheet]]).
- `ingest_jsonl_file`: write a tempfile with mixed lines:
  ```
  {"type":"span","traceId":"a","spanId":"b","name":"x","startTime":1}
  
  not-json
  {"type":"metric","name":"m","dataPoints":[]}
  ```
  Expect `Ok(2)` (the two valid envelopes; blank and unparseable both skipped). Assert two rows in `raw_records`.

Use real instances throughout. There are no trait seams in this module; downstream effects (DB, broadcast) are observed via the same shared in-process state.
