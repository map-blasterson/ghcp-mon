---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/ingest_pipeline.rs`

Covers LLRs:
- [[Each envelope persisted as own raw record]] — `ingest_envelope_writes_one_raw_record_per_envelope`.
- [[OTLP attribute flattening]] — `flatten_otlp_attributes_unwraps_scalars_and_recurses`, `flatten_otlp_attributes_empty_array_yields_empty_map`.
- [[OTLP int value parsed as int64]] — `flatten_otlp_attributes_unwraps_scalars_and_recurses` (asserts `is_i64()` on `intValue:"42"`); `flatten_otlp_attributes_int_value_unparseable_passes_through` covers the MAY-pass-through clause.
- [[Raw request body persisted verbatim per OTLP request]] — `persist_raw_request_writes_verbatim_body`.
- [[Replay parser tags envelopes by type]] — `parse_file_exporter_line_tags_span_metric_log_variants`, `parse_file_exporter_line_returns_bad_request_on_garbage`.
- [[Replay reader skips blank lines]] — `ingest_jsonl_file_skips_blank_and_unparseable_lines`.
- [[Replay reader skips unparseable lines]] — `ingest_jsonl_file_skips_blank_and_unparseable_lines`.
