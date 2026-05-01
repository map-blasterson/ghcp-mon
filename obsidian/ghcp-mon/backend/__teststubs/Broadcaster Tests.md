---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/broadcaster.rs`

Covers LLR:
- [[Broadcaster fan out via tokio broadcast channel]] — `new_with_capacity_and_send_to_subscribers_fans_out`, `send_with_no_subscribers_does_not_panic_or_error`, `capacity_passed_to_underlying_broadcast_channel`.
