---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For each data point of a metric envelope, the normalizer MUST insert one row into `metric_points` carrying `raw_record_id`, `metric_name`, `description`, `unit`, optional `start_unix_ns`/`end_unix_ns`, JSON-encoded `attributes_json` and `value_json`, and `resource_json`/`scope_name`/`scope_version` from the envelope.

## Rationale
Metric data is stored point-by-point so each point is independently queryable.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
