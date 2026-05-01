---
type: cheatsheet
---
Source: `src/static_assets.rs`. Crate path: `ghcp_mon::static_assets`.

## Extract

```rust
use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets; // private

pub async fn static_handler(uri: Uri) -> Response;
```

Behavior surface (observable from tests):
- The asset bundle is `web/dist/` baked at compile time via `rust_embed::RustEmbed`.
- `static_handler(uri)`:
  - Strips a single leading `/` from `uri.path()`. Empty path → `"index.html"`.
  - If the embedded bundle has a file at that path → serve it. `Content-Type` is set from `mime_guess::from_path(path).first_or_octet_stream()`. Body is the file bytes (`file.data.into_owned()` wrapped in `axum::body::Body`). Status defaults to 200.
  - Else if the bundle has `index.html` → serve `index.html` (SPA fallback) with content-type derived from `index.html` (text/html).
  - Else returns `(StatusCode::NOT_FOUND, "not found")`.

`mime_guess` mappings to remember:
- `.html` → `text/html`
- `.js` → `application/javascript` (or `text/javascript` depending on version)
- `.css` → `text/css`
- `.svg` → `image/svg+xml`
- unknown → `application/octet-stream`

## Suggested Test Strategy

- Build a `Uri` via `"/path".parse::<Uri>().unwrap()` and call the handler directly (`#[tokio::test]`). No router needed for unit tests.
- Inspect the returned `Response`:
  - `response.status()`
  - `response.headers().get(header::CONTENT_TYPE)`
  - body via `axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap()`
- The `web/dist/` folder must contain real assets at compile time for the embedded bundle to be non-empty. The crate's `build.rs` likely produces these. Assume `index.html` is present for the SPA-fallback test; if it isn't, the 404 path triggers instead.
- LLR-aligned cases:
  - **Serves embedded asset by path**: pick a path you can guarantee exists at build time, e.g. `"/index.html"`. Assert 200 and a non-empty body.
  - **SPA fallback to index.html**: request a definitely-not-an-asset path like `"/some/route/that/does/not/exist"`. Assert 200 and content-type is `text/html` (because we fell back to `index.html`).
  - **Returns 404 when index missing**: harder — the embedded bundle is fixed at compile time. To test this branch deterministically, factor into an integration test that runs only when `web/dist/` is empty (skipped otherwise), OR test by monkey-patching the embed (not feasible). Pragmatic alternative: assert that requesting a non-existent file under conditions where `index.html` is also absent returns `StatusCode::NOT_FOUND` and body bytes equal `b"not found"` — but this requires a separate fixture build. Document this as a known limitation if the dist folder always contains `index.html`.
  - **Sets content type from extension**: request paths ending in `.css`, `.js`, `.svg`, `.html` (assuming such files exist in `web/dist/` after build) and assert `Content-Type` matches `mime_guess` defaults.
- No async I/O outside axum's body collection. No mocks needed.
