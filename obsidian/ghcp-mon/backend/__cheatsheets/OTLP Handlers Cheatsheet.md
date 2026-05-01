---
type: cheatsheet
---
Source: `src/ingest/otlp.rs`. Crate path: `ghcp_mon::ingest::otlp`.

OTLP/HTTP+JSON receiver handlers. Depends on [[Ingest Pipeline Cheatsheet]] (`ingest_envelope`, `otlp_traces_to_envelopes`, `otlp_metrics_to_envelopes`, `persist_raw_request`), [[Server Router Cheatsheet]] (`AppState`), [[Model Envelope Cheatsheet]] (`Envelope`), [[AppError Cheatsheet]].

## Extract

```rust
use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use bytes::Bytes;
use crate::server::AppState;
use crate::error::AppResult;

pub async fn traces (State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> AppResult<impl IntoResponse>;
pub async fn metrics(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> AppResult<impl IntoResponse>;
pub async fn logs   (State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> AppResult<impl IntoResponse>;

// constants used by the impl:
// const JSON_CT:     &str = "application/json";
// const PROTOBUF_CT: &str = "application/x-protobuf";
```

Behavior surface (no impl bodies, just the contract observable to a tester):
- The handlers read `Content-Type` (lowercased; defaults to `"application/json"` if absent).
- If `Content-Type` contains `application/x-protobuf` → return `AppError::NotImplemented(...)` (HTTP 501 with `{"error": "..."}`).
- Otherwise:
  - UTF-8-decode body; failure → `AppError::BadRequest("utf8: ...")` (HTTP 400).
  - JSON-parse the text into `serde_json::Value`; failure → `AppError::BadRequest("json: ...")` (HTTP 400).
  - **Persist the raw request body** verbatim with `persist_raw_request(pool, "otlp-http-json", Some("application/json"), <record_type>, text)` where:
    - `traces`  → `record_type = "otlp-traces"`
    - `metrics` → `record_type = "otlp-metrics"`
    - `logs`    → `record_type = "otlp-logs"`
  - For `traces`/`metrics`: convert the JSON body into envelopes (`otlp_traces_to_envelopes` / `otlp_metrics_to_envelopes`), then for **each** envelope call `ingest_envelope(state, "otlp-http-json", &serde_json::to_string(&env)?, Envelope::{Span,Metric}(Box::new(env)))`. The handler accumulates a counter `accepted`.
  - For `logs`: persist raw only and return `accepted: 0` (no normalization).
- Success response (all three): `Json(json!({"partialSuccess": {}, "accepted": <count>}))` with status 200.

Routes that mount these handlers (from [[Server Router Cheatsheet]]):
- `POST /v1/traces`
- `POST /v1/metrics`
- `POST /v1/logs`

## Suggested Test Strategy

- Construct an `AppState` with a real pool (`ghcp_mon::db::open(&tmp)`) and a fresh `Broadcaster::new(64)`.
- Mount via `otlp_router(state.clone())` and exercise with `tower::ServiceExt::oneshot` (preferred for routing+layers) — but for unit-level coverage you can call the handlers directly:

  ```rust
  let resp = traces(State(state.clone()), HeaderMap::new(), Bytes::from(body_str)).await?;
  ```

- LLR-aligned cases:
  - **Rejects protobuf content type**: send any of the three handlers a request with `Content-Type: application/x-protobuf` (and any body). Expect `AppError::NotImplemented(_)`. Through the router, status = 501 and JSON body has `error` field starting `"not implemented:"`.
  - **Traces persists raw and normalizes envelopes**: send a JSON body with one or two `resourceSpans → scopeSpans → spans` entries. After the call:
    - Assert exactly 1 row in `raw_records` with `record_type = "otlp-traces"` and `body == <input text>` (verbatim).
    - Assert N additional rows in `raw_records` with `record_type = "span"` (one per derived envelope, written by `ingest_envelope`).
    - Assert response JSON has `"accepted": N`.
  - **Metrics persists raw and normalizes envelopes**: same as above but `record_type = "otlp-metrics"` and per-envelope `record_type = "metric"`.
  - **Logs persisted raw only**: send any valid OTLP-logs JSON shape. Expect `record_type = "otlp-logs"` row written, no `record_type = "log"` rows, and `"accepted": 0`.
  - **Raw request body persisted verbatim**: assert `body` column equals the exact request body string (including any whitespace) for the "raw" row. Use a deliberately-formatted JSON with extra whitespace and confirm bytewise equality.
- For body-limit testing, see [[Server Router Cheatsheet]] (the 64 MiB cap is enforced at the router layer, not in these handlers).
- No mocks. Watch out for: handlers do **not** swallow normalization errors — if `ingest_envelope` returns `Err`, the handler short-circuits with that error. To test happy paths reliably, ensure your fixture span/metric envelopes round-trip through `parse_file_exporter_line` (well-formed `Envelope`).
