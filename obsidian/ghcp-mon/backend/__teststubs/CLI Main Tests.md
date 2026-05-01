---
type: test-stub
status: partial
tags:
  - test/generated
---
Test file: `tests/cli_main.rs` (subprocess tests via `env!("CARGO_BIN_EXE_ghcp-mon")`)

Covers LLRs (exit-code + `--help` parsing + listener-bind probes):
- [[CLI db option default path]] — `cli_db_default_path_visible_in_help`.
- [[CLI defines serve and replay subcommands]] — `cli_help_lists_serve_and_replay_subcommands`, `cli_unknown_subcommand_exits_non_zero`.
- [[CLI initializes tracing subscriber]] — `cli_initializes_tracing_subscriber_emits_to_stderr` (smoke; observable side effects of tracing init are limited from a black-box test).
- [[CLI session state dir flag overrides default]] — `cli_session_state_dir_flag_visible_in_help`.
- [[Replay inline mode ingests in-process]] — `replay_inline_ingests_into_db_in_process`.
- [[Replay non-inline posts to running server]] — `replay_non_inline_help_documents_server_option` (PARTIAL — full e2e would require running an in-test server; the CLI surface is verified instead).
- [[Serve binds OTLP and API listeners]] — `serve_binds_otlp_and_api_listeners_on_configured_addrs` (spawns the binary, probes both ports).

Notes / partial coverage:
- The tracing-subscriber test is a startup smoke check; verifying the exact env-filter fallback (`info,sqlx=warn,...`) from a black-box would require parsing tracing-formatted stderr for a long-running command. Acceptable per cheatsheet guidance.
- The non-inline replay test stops at CLI surface verification because the full e2e requires spinning up a separate server process or in-test axum server, which adds flakiness beyond this scope.
