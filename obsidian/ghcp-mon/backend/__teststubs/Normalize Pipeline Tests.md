---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/normalize_pipeline.rs`

All tests use a real `sqlx::SqlitePool` (via `db::open`), real `Broadcaster`, and assert table state + bus events post-`handle_envelope`.

Covers LLRs:
- [[Span upsert by trace and span id]] — `span_upsert_keyed_by_trace_and_span_id_inserts_real_state`, `span_upsert_on_conflict_forces_real_state_and_coalesces_optional_fields`.
- [[Placeholder span for unseen parent]] — `placeholder_inserted_for_unseen_parent`, `placeholder_creation_is_idempotent_across_reingest`.
- [[Placeholder upgrade preserved across reingest]] — `placeholder_upgrade_flips_ingestion_state_to_real_with_upgrade_action`, `placeholder_upgrade_preserved_across_reingest_does_not_regress`.
- [[Span normalize emits span and trace events]] — `span_normalize_emits_span_and_trace_events_with_action_insert`.
- [[Placeholder creation emits placeholder events]] — `placeholder_creation_emits_placeholder_events_only_when_inserted`.
- [[Span events idempotently replaced on span upsert]] — `span_events_idempotently_replaced_on_reingest`.
- [[Invoke agent span upserts agent run]] — `invoke_agent_span_upserts_agent_run`, `invoke_agent_falls_back_to_name_suffix_for_agent_name`.
- [[Chat span upserts chat turn]] — `chat_span_upserts_chat_turn_with_token_counters`, `chat_span_prefers_request_model_over_response_model`.
- [[Chat token usage attributes create context snapshot]] — `chat_token_usage_creates_chat_span_context_snapshot`.
- [[Execute tool span upserts tool call]] — `execute_tool_span_upserts_tool_call`.
- [[External tool span upserts external tool call]] — `external_tool_span_upserts_external_tool_call_with_fallback_attrs`.
- [[External tool paired to internal tool call by call id]] — `external_tool_paired_to_internal_tool_call_by_call_id`.
- [[Projection upserts emit derived events]] — `projection_upserts_emit_derived_events`.
- [[Session upserted per conversation id]] — `session_upserted_per_conversation_id_with_min_max_timestamps`.
- [[Session counters refreshed on session upsert]] — `session_counters_refreshed_on_session_upsert`.
- [[Session upsert emits derived session event]] — `session_upsert_emits_derived_session_update_event`.
- [[Effective conversation id inherited from ancestors]] — `effective_conversation_id_inherited_from_ancestors`.
- [[Projection pointers resolved via ancestor walk]] — `projection_pointers_resolved_via_ancestor_walk`.
- [[Forward resolve descendants on parent arrival]] — `forward_resolve_descendants_on_parent_arrival`.
- [[Hook start event derives hook invocation]] — `hook_start_event_derives_hook_invocation`.
- [[Hook end event completes hook invocation]] — `hook_end_event_completes_hook_invocation_with_duration`.
- [[Skill invoked event records skill invocation]] — `skill_invoked_event_records_skill_invocation_idempotently`.
- [[Usage info event creates context snapshot]] — `usage_info_event_creates_context_snapshot_with_event_source`.
- [[Chat turn tool count refreshed]] — `chat_turn_tool_count_refreshed_only_for_internal_tool_calls`.
- [[Metric data points persisted to metric_points]] — `metric_data_points_persisted_to_metric_points_with_event_emission`.
- [[Metric ingest emits raw metric event]] — `metric_data_points_persisted_to_metric_points_with_event_emission`.
- [[Logs not normalized currently]] — `logs_envelope_is_no_op`.
