# ingest — Memory Bank (backend)

## Purpose
The `src/ingest/` area is the entry point for telemetry into ghcp-mon. It accepts OTLP/HTTP JSON requests on `/v1/{traces,metrics,logs}`, accepts file-exporter JSONL replays via `POST /api/replay` (and the `replay --inline` CLI mode), persists every received payload verbatim into `raw_records`, converts OTLP wire shapes into the internal `Envelope` form, and hands each envelope to `normalize::ingest_envelope` for projection into the relational schema.

## Key behaviors

### OTLP receiver
The receiver implements three POST endpoints with parallel structure but divergent normalization behavior, all consuming OTLP/JSON only.

- **Content-type gating.** When the request `Content-Type` header (case-insensitively) contains `application/x-protobuf`, the receiver returns HTTP 501 Not Implemented with a JSON error body explaining that protobuf ingestion is not implemented. Only OTLP/JSON is supported; protobuf clients fail loudly rather than silently mis-parsing. (`backend/llr/OTLP rejects protobuf content type.md`)
- **`POST /v1/traces`.** Persists the raw request body once with `record_type='otlp-traces'` and `source='otlp-http-json'`, converts each span into a `SpanEnvelope`, calls `ingest_envelope` per envelope, and responds with `{"partialSuccess": {}, "accepted": <n>}` where `<n>` is the count of envelopes ingested. (`backend/llr/OTLP traces persists raw and normalizes envelopes.md`)
- **`POST /v1/metrics`.** Persists the raw body once with `record_type='otlp-metrics'`, converts each metric into a `MetricEnvelope` (one envelope per metric, carrying its data-point list), calls `ingest_envelope` per envelope, and responds with `{"partialSuccess": {}, "accepted": <n>}`. Mirror of the traces path so dashboards correlate metrics with the same raw-record audit trail. (`backend/llr/OTLP metrics persists raw and normalizes envelopes.md`)
- **`POST /v1/logs`.** Persists the raw body with `record_type='otlp-logs'` and responds with `{"partialSuccess": {}, "accepted": 0}` *without* producing any normalized rows or envelopes. Logs are archived but not yet normalized; the explicit `accepted: 0` signals to clients that no derived rows were created. (`backend/llr/OTLP logs persisted raw only.md`)

All three endpoints share the same audit guarantee: the raw request body is written before any envelope derivation occurs.

### Raw archival
Two layers of raw persistence ensure both the original wire request and each derived envelope are reconstructible.

- **Per-request raw record.** For every OTLP/HTTP request the receiver accepts, the handler writes exactly one `raw_records` row with `source='otlp-http-json'` and `content_type='application/json'`, *before* deriving any envelopes. This preserves the wire bytes for audit and replay-from-archive regardless of how many envelopes the request produced. (`backend/llr/Raw request body persisted verbatim per OTLP request.md`)
- **Per-envelope raw record.** `ingest_envelope` inserts one additional `raw_records` row per envelope, with `record_type` set to the envelope type tag and `content_type='application/json'`, before normalizing. Each normalized row references its own raw-record id, giving every projection a direct backlink to the JSON that produced it. (`backend/llr/Each envelope persisted as own raw record.md`)

The `record_type` namespace distinguishes the two layers: `otlp-traces`/`otlp-metrics`/`otlp-logs` for the request layer; per-envelope tags (span/metric/log) at the envelope layer.

### Replay (HTTP + JSONL reader)
Replay ingests file-exporter JSONL files (one envelope per line) so operators can reconstruct historical sessions or feed deterministic test fixtures. (`backend/hlr/File Exporter Replay.md`)

- **HTTP endpoint.** `POST /api/replay` accepts a JSON body `{"path": "<filesystem path>"}`, runs `ingest_jsonl_file` against that path with `source='replay'`, and returns `{"path": "<input path>", "ingested": <count>}` on success. The endpoint is the bridge between the CLI's non-inline mode and the live server. (`backend/llr/Replay endpoint accepts path and returns count.md`)
- **Blank-line tolerance.** `ingest_jsonl_file` skips lines whose trimmed content is empty without incrementing the ingest counter and without raising an error. Tolerates trailing newlines and blank separators in exporter output. (`backend/llr/Replay reader skips blank lines.md`)
- **Unparseable-line tolerance.** When a line fails to parse as a file-exporter envelope, `ingest_jsonl_file` logs a warning and continues processing subsequent lines instead of aborting the replay. A single corrupt line in a long telemetry file does not invalidate the rest. (`backend/llr/Replay reader skips unparseable lines.md`)
- **Envelope tagging.** `parse_file_exporter_line` deserializes a JSON object whose `type` field is one of `"span"`, `"metric"`, `"log"` into the corresponding `Envelope::Span`/`Envelope::Metric`/`Envelope::Log` variant, returning `AppError::BadRequest` if the input does not parse as JSON at all. The file-exporter format uses an externally-tagged discriminant to distinguish kinds. The `Envelope` enum itself is co-sourced in `src/model.rs`. (`backend/llr/Replay parser tags envelopes by type.md`)

### Envelope conversion
OTLP wire shapes are reshaped into the internal envelope form before reaching `normalize`.

- **Attribute flattening.** `flatten_otlp_attributes` converts an OTLP attributes array (each element `{key, value: AnyValue}`) into a flat JSON object keyed by attribute key, with values unwrapped from `AnyValue`: `stringValue`, `intValue`, `doubleValue`, `boolValue`, `bytesValue`, `arrayValue.values` (recursively flattened), `kvlistValue.values` (recursively flattened). Internal envelopes use the flat map so downstream code can index attributes uniformly. (`backend/llr/OTLP attribute flattening.md`)
- **Int64 string parsing.** When an `AnyValue.intValue` is encoded as a string (per OTLP/JSON, which serializes int64 as a decimal string), the flatten routine parses it as `i64` and emits a JSON number. If parsing fails it MAY pass the original string through unchanged. Downstream queries expect numeric values. (`backend/llr/OTLP int value parsed as int64.md`)

## Public surface
- `POST /v1/traces` — body: OTLP/JSON `ExportTraceServiceRequest`; response: `{"partialSuccess": {}, "accepted": <n>}` where `<n>` = spans ingested.
- `POST /v1/metrics` — body: OTLP/JSON `ExportMetricsServiceRequest`; response: `{"partialSuccess": {}, "accepted": <n>}` where `<n>` = metrics ingested (one envelope per metric).
- `POST /v1/logs` — body: OTLP/JSON `ExportLogsServiceRequest`; response: always `{"partialSuccess": {}, "accepted": 0}` (raw archival only).
- All three return HTTP 501 with a JSON error body when `Content-Type` contains `application/x-protobuf`.
- `POST /api/replay` — body `{"path": "<fs path>"}`; response `{"path": "<fs path>", "ingested": <count>}`.
- `ingest_envelope(envelope)` — the chokepoint into `normalize`. Writes the per-envelope raw row, then normalizes.
- `ingest_jsonl_file(path, source)` — used by both the replay HTTP handler (with `source='replay'`) and by `replay --inline` invoked from `main.rs`.
- `parse_file_exporter_line(line)` — `&str -> Result<Envelope, AppError>`; `AppError::BadRequest` on JSON parse failure.
- `flatten_otlp_attributes(attrs)` — OTLP `AnyValue`-array → flat JSON map.

## Invariants & constraints
- **JSON-only OTLP.** Protobuf content type → HTTP 501 with JSON error body; no silent fallback. (`OTLP rejects protobuf content type`)
- **Verbatim raw archival per request.** Exactly one `raw_records` row per accepted OTLP/HTTP request, written before any envelope derivation, with `source='otlp-http-json'` and `content_type='application/json'`. (`Raw request body persisted verbatim per OTLP request`)
- **Per-envelope raw record.** Every envelope produces its own `raw_records` row before normalization, so each normalized row links back to the JSON that produced it. (`Each envelope persisted as own raw record`)
- **Logs are archive-only.** `/v1/logs` never normalizes and always returns `accepted: 0`. (`OTLP logs persisted raw only`)
- **Idempotent re-delivery.** Re-ingesting the same telemetry is safe; idempotence is enforced by `normalize`-side upsert keys, not by `ingest`.
- **Replay reader robustness.**
  - Blank/whitespace-only lines: skipped silently, counter not incremented. (`Replay reader skips blank lines`)
  - Lines that fail file-exporter envelope parse: warning logged, processing continues. (`Replay reader skips unparseable lines`)
  - Lines whose JSON is itself invalid bubble up as `AppError::BadRequest` from `parse_file_exporter_line`. (`Replay parser tags envelopes by type`)
- **OTLP/JSON int64 strings.** Flattener parses string-encoded `intValue` as `i64` and emits a JSON number; falls back to passthrough on parse failure. (`OTLP int value parsed as int64`)
- **Flat attribute shape.** `AnyValue` (`stringValue`/`intValue`/`doubleValue`/`boolValue`/`bytesValue`/`arrayValue`/`kvlistValue`) → flat JSON map; `arrayValue` and `kvlistValue` recurse. (`OTLP attribute flattening`)

## Dependencies
- **Routing.** All HTTP endpoints in this area are mounted by `src/server.rs`. The OTLP body limit of **64 MiB** is enforced at the router layer in `server.rs`, *not* in `ingest/`.
- **Envelope tag enum.** The `Envelope::{Span,Metric,Log}` discriminator that `parse_file_exporter_line` matches against lives in `src/model.rs` (the `Replay parser tags envelopes by type` LLR is co-sourced there).
- **Database writes.** Both raw layers (per-request and per-envelope) call into the `db/` DAO (`insert_raw`-style operations) to write `raw_records`.
- **Normalize handoff.** `ingest_envelope` is the single chokepoint that crosses from `ingest/` into `normalize/`. Idempotency of re-delivery is delegated downstream.
- **Errors.** All fallible handlers and helpers return through `AppError` (uniform error reporting); replay parse failures specifically return `AppError::BadRequest`.
- **CLI integration.** `ingest_jsonl_file` is invoked directly from `main.rs` when the CLI is run as `replay --inline`, bypassing the HTTP endpoint.

## Where to read for detail

### HLRs (3)
- `backend/hlr/OTLP HTTP Receiver.md` — OTLP/JSON ingestion umbrella; lists routing, content-type gating, attribute flattening, int64 parsing, and per-signal raw+normalize behavior.
- `backend/hlr/Telemetry Persistence.md` — raw and normalized persistence umbrella; the source of both raw-archival LLRs in this area.
- `backend/hlr/File Exporter Replay.md` — JSONL replay umbrella; covers the replay endpoint and reader robustness.

### LLRs (12)
- `backend/llr/OTLP traces persists raw and normalizes envelopes.md`
- `backend/llr/OTLP metrics persists raw and normalizes envelopes.md`
- `backend/llr/OTLP logs persisted raw only.md` *(note: vault filename uses "persisted", not "persists")*
- `backend/llr/OTLP rejects protobuf content type.md`
- `backend/llr/OTLP attribute flattening.md`
- `backend/llr/OTLP int value parsed as int64.md`
- `backend/llr/Raw request body persisted verbatim per OTLP request.md`
- `backend/llr/Each envelope persisted as own raw record.md`
- `backend/llr/Replay endpoint accepts path and returns count.md`
- `backend/llr/Replay reader skips blank lines.md`
- `backend/llr/Replay reader skips unparseable lines.md`
- `backend/llr/Replay parser tags envelopes by type.md` *(co-sourced in `src/model.rs`)*

### Source
- `src/ingest/mod.rs` — `ingest_envelope` chokepoint, `ingest_jsonl_file` (used by HTTP replay and `replay --inline`), shared types.
- `src/ingest/otlp.rs` — `/v1/traces`, `/v1/metrics`, `/v1/logs` handlers; protobuf rejection; `flatten_otlp_attributes`; OTLP→envelope conversion; int64-string parsing.
- `src/ingest/replay.rs` — `POST /api/replay` handler; `parse_file_exporter_line`; blank/unparseable-line tolerance.

### Cross-area touch points
- `src/server.rs` — mounts the routes; enforces OTLP 64 MiB body limit.
- `src/model.rs` — `Envelope` enum (`Span` / `Metric` / `Log`); co-source for `Replay parser tags envelopes by type`.
- `src/db/` — DAO target for both raw layers.
- `src/normalize/` — receives every envelope via `ingest_envelope`; owns idempotent upsert.
- `src/main.rs` — invokes `ingest_jsonl_file` directly under `replay --inline`.