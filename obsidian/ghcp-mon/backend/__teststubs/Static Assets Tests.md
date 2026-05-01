---
type: test-stub
status: partial
tags:
  - test/generated
  - test/blocked
---
Test file: `tests/static_assets.rs`

Covers LLRs:
- [[Static handler serves embedded asset by path]] — `serves_index_html_at_root`, `serves_explicit_asset_path`.
- [[Static handler SPA fallback to index html]] — `spa_fallback_for_unknown_route` (asserts text/html when bundle has index.html, else 404 — both branches valid).
- [[Static handler sets content type from extension]] — `content_type_reflects_extension_when_asset_present` (assertion only fires when the asset exists in the embedded bundle, since the file list is fixed at compile time and we are source-blind).
- [[Static handler returns 404 when index missing]] — **BLOCKED.** The embedded `web/dist/` bundle is fixed at compile time. To deterministically exercise the "no index.html" branch we would need an alternate compile-time bundle, which is out of scope for source-blind test generation. The other tests degrade gracefully to 404 if the bundle is empty.

Unblocking: factor `static_handler` to take an injectable `RustEmbed`-like trait, or provide a build-time feature flag that swaps to an empty bundle for tests.
