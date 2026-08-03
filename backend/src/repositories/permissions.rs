//! 权限 Repository：权限组/权限查询 + 仪表盘统计（只读查询为主）

use crate::models::{DashboardStatsRow, PermissionGroupRow, PermissionRow};
use sqlx::PgPool;

pub async fn list_groups(pool: &PgPool) -> Result<Vec<PermissionGroupRow>, sqlx::Error> {
    sqlx::query_as::<_, PermissionGroupRow>(
        "SELECT id, code, name, icon FROM permission_groups ORDER BY sort_order, id",
    )
    .fetch_all(pool)
    .await
}

pub async fn list_permissions(pool: &PgPool) -> Result<Vec<PermissionRow>, sqlx::Error> {
    sqlx::query_as::<_, PermissionRow>(
        "SELECT id, group_id, code, name, type, description
         FROM permissions ORDER BY sort_order, id",
    )
    .fetch_all(pool)
    .await
}

pub async fn permission_codes_by_user(
    pool: &PgPool,
    user_id: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT p.code FROM permissions p
         JOIN role_permissions rp ON rp.permission_id = p.id
         JOIN user_roles ur ON ur.role_id = rp.role_id
         WHERE ur.user_id = $1
         ORDER BY p.code",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn stats(pool: &PgPool) -> Result<DashboardStatsRow, sqlx::Error> {
    sqlx::query_as::<_, DashboardStatsRow>(
        "SELECT
            (SELECT count(*) FROM users WHERE deleted_at IS NULL) AS total_users,
            (SELECT count(*) FROM users WHERE deleted_at IS NULL AND status = 'active') AS active_users,
            (SELECT count(*) FROM roles) AS total_roles,
            (SELECT count(*) FROM permissions) AS total_permissions,
            (SELECT count(*) FROM users WHERE deleted_at IS NULL AND status = 'suspended') AS suspended_users",
    )
    .fetch_one(pool)
    .await
}
