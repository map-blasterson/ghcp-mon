//! Tests for AppError → HTTP response mapping. LLRs:
//! - AppError JSON body contains error message
//! - AppError maps variants to status codes
//! - AppError converts from sqlx serde io migrate

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use ghcp_mon::error::AppError;
use serde_json::Value;
use assert_matches::assert_matches;

async fn render(err: AppError) -> (StatusCode, Value) {
    let resp = err.into_response();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
    let v: Value = serde_json::from_slice(&bytes).expect("body must be JSON");
    (status, v)
}

#[tokio::test]
async fn into_response_bad_request_maps_to_400() {
    let (status, body) = render(AppError::BadRequest("oops".into())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], Value::String("oops".into()));
}

#[tokio::test]
async fn into_response_not_found_maps_to_404() {
    let (status, body) = render(AppError::NotFound).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], Value::String("not found".into()));
}

#[tokio::test]
async fn into_response_not_implemented_maps_to_501() {
    let (status, body) = render(AppError::NotImplemented("nope".into())).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(body["error"], Value::String("nope".into()));
}

#[tokio::test]
async fn into_response_other_maps_to_500() {
    let (status, body) = render(AppError::Other("boom".into())).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    // 500-mapped variants render via Display (`other: boom`).
    assert_eq!(body["error"], Value::String("other: boom".into()));
}

#[tokio::test]
async fn into_response_sqlx_maps_to_500() {
    let (status, body) = render(AppError::Sqlx(sqlx::Error::RowNotFound)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().starts_with("sqlx error:"));
}

#[tokio::test]
async fn into_response_json_maps_to_500() {
    let json_err = serde_json::from_str::<u8>("not-json").unwrap_err();
    let (status, body) = render(AppError::Json(json_err)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().starts_with("json error:"));
}

#[tokio::test]
async fn into_response_io_maps_to_500() {
    let io = std::io::Error::new(std::io::ErrorKind::Other, "x");
    let (status, body) = render(AppError::Io(io)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().starts_with("io error:"));
}

#[test]
fn from_sqlx_error_via_question_mark() {
    fn inner() -> Result<(), AppError> {
        let r: Result<(), sqlx::Error> = Err(sqlx::Error::RowNotFound);
        r?;
        Ok(())
    }
    let err = inner().unwrap_err();
    assert_matches!(err, AppError::Sqlx(_));
}

#[test]
fn from_serde_json_error_via_question_mark() {
    fn inner() -> Result<(), AppError> {
        let _: u8 = serde_json::from_str("nope")?;
        Ok(())
    }
    assert_matches!(inner().unwrap_err(), AppError::Json(_));
}

#[test]
fn from_io_error_via_question_mark() {
    fn inner() -> Result<(), AppError> {
        let r: Result<(), std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::Other, "x"));
        r?;
        Ok(())
    }
    assert_matches!(inner().unwrap_err(), AppError::Io(_));
}

#[test]
fn from_migrate_error_via_question_mark() {
    fn inner() -> Result<(), AppError> {
        let m: sqlx::migrate::MigrateError = sqlx::migrate::MigrateError::VersionMismatch(0);
        Err::<(), _>(m)?;
        Ok(())
    }
    assert_matches!(inner().unwrap_err(), AppError::Migrate(_));
}
