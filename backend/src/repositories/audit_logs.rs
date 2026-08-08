//! 审计日志 Repository：安全变更记录与只读分页查询。

use crate::models::AuditLogRow;
use serde_json::Value;
use sqlx::{PgConnection, PgPool};

pub async fn record(
    connection: &mut PgConnection,
    actor_user_id: Option<i64>,
    action: &str,
    target_type: &str,
    target_id: Option<i64>,
    details: Value,
) -> Result<(), sqlx::Error> {
    let trace_id = crate::telemetry::current_trace_id();
    sqlx::query(
        "INSERT INTO audit_logs
             (actor_user_id, action, target_type, target_id, details, trace_id)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(actor_user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(details)
    .bind(trace_id)
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn list(
    pool: &PgPool,
    keyword: Option<String>,
    action: Option<String>,
    page: i64,
    page_size: i64,
) -> Result<Vec<AuditLogRow>, sqlx::Error> {
    sqlx::query_as::<_, AuditLogRow>(
        "SELECT a.id, a.actor_user_id, u.username AS actor_username, a.action,
                a.target_type, a.target_id, a.details, a.trace_id, a.created_at
         FROM audit_logs a
         LEFT JOIN users u ON u.id = a.actor_user_id
         WHERE ($1::text IS NULL OR a.action ILIKE '%' || $1 || '%'
                OR a.target_type ILIKE '%' || $1 || '%'
                OR COALESCE(u.username, '') ILIKE '%' || $1 || '%'
                OR a.trace_id = $1)
           AND ($2::text IS NULL OR a.action = $2)
         ORDER BY a.created_at DESC, a.id DESC
         LIMIT $3 OFFSET $4",
    )
    .bind(keyword)
    .bind(action)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(pool)
    .await
}

pub async fn count(
    pool: &PgPool,
    keyword: Option<String>,
    action: Option<String>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*)
         FROM audit_logs a
         LEFT JOIN users u ON u.id = a.actor_user_id
         WHERE ($1::text IS NULL OR a.action ILIKE '%' || $1 || '%'
                OR a.target_type ILIKE '%' || $1 || '%'
                OR COALESCE(u.username, '') ILIKE '%' || $1 || '%'
                OR a.trace_id = $1)
           AND ($2::text IS NULL OR a.action = $2)",
    )
    .bind(keyword)
    .bind(action)
    .fetch_one(pool)
    .await
}
