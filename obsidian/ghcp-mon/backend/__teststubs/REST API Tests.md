---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/rest_api.rs`

Covers LLRs:
- [[API healthz endpoint]] — `healthz_returns_200_ok_true`.
- [[API router exposes session and span endpoints]] — `router_exposes_documented_endpoints_not_404`.
- [[API allows any origin via CORS]] — `cors_layer_allows_any_origin`.
- [[API list sessions ordered by recency]] — `list_sessions_orders_by_last_seen_desc_and_clamps_limit`, `list_sessions_filtered_by_since`.
- [[API list query limit clamped]] — `list_sessions_limit_zero_clamped_to_one`, `list_sessions_limit_clamped_to_max_500`, `list_spans_default_limit_clamped_max_1000`.
- [[API list sessions enriched with local workspace metadata]] — `list_sessions_includes_local_metadata_when_yaml_present`, `list_sessions_local_metadata_null_when_yaml_missing`.
- [[API session detail returns span count]] — `get_session_404_when_missing`, `get_session_returns_span_count`.
- [[API session detail enriched with local workspace metadata]] — `get_session_includes_local_metadata`.
- [[API delete session purges traces and projections]] — `delete_session_404_when_missing`, `delete_session_purges_rows_and_emits_derived_event`.
- [[API list session contexts ordered by capture]] — `list_session_contexts_ordered_by_captured_ns_asc`.
- [[API list spans filterable by session and kind]] — `list_spans_filterable_by_kind_class`.
- [[API get span returns events parent children projection]] — `get_span_404_when_missing`, `get_span_returns_span_events_parent_children_projection`.
- [[API list traces aggregates per trace]] — `list_traces_aggregates_per_trace_with_kind_counts`.
- [[API list traces floats placeholder only traces]] — `list_traces_floats_placeholder_only_traces_to_top`.
- [[API get trace returns span tree]] — `get_trace_404_when_no_spans`, `get_trace_returns_tree_shape`.
- [[API session span tree trace scoped union]] — `session_span_tree_unions_by_trace_id_with_seeds`.
- [[API list raw filterable by record type]] — `list_raw_filterable_by_record_type_and_body_parsed_when_json`.

Notes / partial coverage:
- Span-tree placeholder-floating ordering is not separately asserted (the union test fixes ordering with timestamps); enhance later if this becomes a regression source.
- `/api/sessions/:cid/registries` is mounted by the router test but no LLR specifies its body — out of scope.
