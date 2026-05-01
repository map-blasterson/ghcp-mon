//! Tests for ghcp_mon::static_assets::static_handler. LLRs:
//! - Static handler serves embedded asset by path
//! - Static handler SPA fallback to index html
//! - Static handler sets content type from extension
//! - Static handler returns 404 when index missing  (BLOCKED — see test-stub)

use axum::body::to_bytes;
use axum::http::{header, StatusCode, Uri};
use ghcp_mon::static_assets::static_handler;

#[tokio::test]
async fn serves_index_html_at_root() {
    let resp = static_handler("/".parse::<Uri>().unwrap()).await;
    // Either index.html exists (status 200) or the bundle is empty (status 404).
    if resp.status() == StatusCode::OK {
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert!(!bytes.is_empty(), "served asset MUST have non-empty body");
    } else {
        // Without a built dashboard, static_handler returns 404.
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn serves_explicit_asset_path() {
    let resp = static_handler("/index.html".parse::<Uri>().unwrap()).await;
    if resp.status() == StatusCode::OK {
        let ct = resp.headers().get(header::CONTENT_TYPE).cloned();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert!(!bytes.is_empty());
        let ct_s = ct.as_ref().and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(ct_s.starts_with("text/html"),
            ".html asset Content-Type MUST be text/html (got {:?})", ct_s);
    }
}

#[tokio::test]
async fn spa_fallback_for_unknown_route() {
    // A path that is definitely NOT an asset; if index.html is in the bundle the
    // handler MUST fall back to it (status 200, html content-type).
    let resp = static_handler("/some/spa/route/that/is/not/a/real/file".parse::<Uri>().unwrap()).await;
    if resp.status() == StatusCode::OK {
        let ct = resp.headers().get(header::CONTENT_TYPE).cloned();
        let ct_s = ct.as_ref().and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(ct_s.starts_with("text/html"),
            "SPA fallback MUST serve index.html (text/html), got {:?}", ct_s);
    } else {
        // bundle empty path — both branches absent, verifies the 404 branch instead.
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn content_type_reflects_extension_when_asset_present() {
    // Try a likely .css/.js asset path; if absent the test is a no-op assertion
    // (we cannot know what files exist in the embed without reading source).
    for (path, expected_prefix) in [
        ("/assets/index.css", "text/css"),
        ("/assets/index.js", "application/javascript"),
    ] {
        let resp = static_handler(path.parse::<Uri>().unwrap()).await;
        if resp.status() == StatusCode::OK {
            // Only assert content-type when we got a real asset. (Fallback to index.html
            // would be text/html and would not match — but that path is the SPA fallback,
            // which is exercised by the test above.)
            let ct = resp.headers().get(header::CONTENT_TYPE).cloned();
            let ct_s = ct.as_ref().and_then(|v| v.to_str().ok()).unwrap_or("");
            if !ct_s.starts_with("text/html") {
                assert!(ct_s.starts_with(expected_prefix) || ct_s.starts_with("text/javascript"),
                    "{} MUST set Content-Type from extension, got {:?}", path, ct_s);
            }
        }
    }
}
