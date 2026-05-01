---
type: cheatsheet
---
Source: `src/error.rs`. Crate: `ghcp-mon` (edition 2021). Library boundary is `ghcp_mon::error`.

## Extract

```rust
use thiserror::Error;
use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};

#[derive(Debug, Error)]
pub enum AppError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("other: {0}")]
    Other(String),
}

pub type AppResult<T> = std::result::Result<T, AppError>;

impl IntoResponse for AppError { fn into_response(self) -> Response; }
```

Path: `ghcp_mon::error::{AppError, AppResult}`.

Key facts for tests:
- `#[from]` on `Sqlx`, `Migrate`, `Json`, `Io` — those error types convert with `?` / `.into()`.
- `BadRequest`, `NotImplemented`, `Other` carry a `String` payload.
- `NotFound` is unit.
- `IntoResponse` produces an `axum::response::Response` whose body is `Json(json!({"error": <msg>}))`.

## Suggested Test Strategy

- Construct each variant directly and call `.into_response()`. Inspect the resulting `Response`:
  - Status code via `response.status()`.
  - Body via `axum::body::to_bytes(response.into_body(), usize::MAX).await` then parse as `serde_json::Value` and read `["error"]`.
- For `From` conversions: build a real underlying error (e.g. `sqlx::Error::RowNotFound`, `serde_json::from_str::<u8>("oops").unwrap_err()`, `std::io::Error::new(...)`, `sqlx::migrate::MigrateError::from(std::io::Error::...)`) and assert the converted `AppError` matches the expected variant — `assert_matches!(err, AppError::Sqlx(_))` works well.
- For status-mapping tests, the only variants that map to specific codes are `BadRequest` → 400, `NotFound` → 404, `NotImplemented` → 501; everything else is 500. The 500-mapped variants render their `Display` (`to_string()`) into the JSON body, while `BadRequest`/`NotImplemented` render the inner `String` and `NotFound` renders the literal `"not found"`.
- Use `http_body_util::BodyExt::collect` (already a dev-dep) or `axum::body::to_bytes` to drain the response body in async tests with `#[tokio::test]`.
