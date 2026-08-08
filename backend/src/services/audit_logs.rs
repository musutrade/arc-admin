//! 审计日志服务：分页参数与筛选校验。

use crate::access::ActorContext;
use crate::error::{db_error, ApiError};
use crate::models::{audit_log_response, AuditLogQuery, PageAuditLog};
use crate::repositories;
use sqlx::PgPool;

pub async fn list(
    pool: &PgPool,
    actor: &ActorContext,
    query: &AuditLogQuery,
) -> Result<PageAuditLog, ApiError> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let keyword = query
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let action = query
        .action
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let rows = repositories::audit_logs::list(
        pool,
        actor,
        keyword.clone(),
        action.clone(),
        page,
        page_size,
    )
    .await
    .map_err(db_error)?;
    let total = repositories::audit_logs::count(pool, actor, keyword, action)
        .await
        .map_err(db_error)?;
    Ok(PageAuditLog {
        items: rows.into_iter().map(audit_log_response).collect(),
        total,
        page,
        page_size,
    })
}
