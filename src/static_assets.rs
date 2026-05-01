//! Embedded SPA assets.
//!
//! The Vite build output (`web/dist/`) is baked into the binary at compile
//! time. The `static_handler` serves any matching file and falls back to
//! `index.html` for unknown paths so client-side routes work.
use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct Assets;

pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = Assets::get(path) {
        return serve(path, file);
    }

    // SPA fallback — anything that isn't an asset returns index.html so the
    // client-side router can handle it. API/WS routes are matched before this
    // fallback, so they are unaffected.
    if let Some(file) = Assets::get("index.html") {
        return serve("index.html", file);
    }

    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn serve(path: &str, file: rust_embed::EmbeddedFile) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(file.data.into_owned()))
        .unwrap()
}
