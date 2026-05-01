---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/app_error.rs`

Covers LLRs:
- [[AppError JSON body contains error message]] — every `into_response_*` test verifies status code AND `{"error": "..."}` body shape per variant.
- [[AppError maps variants to status codes]] — `into_response_bad_request_maps_to_400`, `into_response_not_found_maps_to_404`, `into_response_not_implemented_maps_to_501`, `into_response_other_maps_to_500`, `into_response_sqlx_maps_to_500`, `into_response_json_maps_to_500`, `into_response_io_maps_to_500`.
- [[AppError converts from sqlx serde io migrate]] — `from_sqlx_error_via_question_mark`, `from_serde_json_error_via_question_mark`, `from_io_error_via_question_mark`, `from_migrate_error_via_question_mark`.
