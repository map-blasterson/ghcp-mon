//! Thin DAO helpers. Most queries elsewhere use sqlx::query directly with the
//! shared pool; this module hosts a few repeated insert/lookup helpers.

use sqlx::SqlitePool;

pub async fn insert_raw(
    pool: &SqlitePool,
    source: &str,
    record_type: &str,
    content_type: Option<&str>,
    body: &str,
) -> sqlx::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO raw_records(source, record_type, content_type, body) VALUES (?,?,?,?) RETURNING id"
    )
    .bind(source)
    .bind(record_type)
    .bind(content_type)
    .bind(body)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}
