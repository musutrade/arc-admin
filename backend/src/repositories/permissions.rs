//! 权限 Repository：权限组/权限查询 + 仪表盘统计（只读查询为主）

use crate::models::{DashboardStatsRow, PermissionGroupRow, PermissionRow};
use sqlx::PgPool;

pub async fn auth_context(
    pool: &PgPool,
    user_id: i64,
) -> Result<(bool, i64, Vec<String>), sqlx::Error> {
    sqlx::query_as::<_, (bool, i64, Vec<String>)>(
        "SELECT
            EXISTS(
                SELECT 1 FROM users
                WHERE id = $1 AND deleted_at IS NULL AND status = 'active'
            ) AS active,
            COALESCE(max(u.token_version), -1) AS token_version,
            COALESCE(
                array_agg(DISTINCT p.code ORDER BY p.code)
                    FILTER (WHERE p.code IS NOT NULL),
                ARRAY[]::text[]
            ) AS permission_codes
         FROM users u
         LEFT JOIN user_roles ur ON ur.user_id = u.id
         LEFT JOIN roles r ON r.id = ur.role_id AND r.is_active = TRUE
         LEFT JOIN role_permissions rp ON rp.role_id = r.id
         LEFT JOIN permissions p ON p.id = rp.permission_id
         WHERE u.id = $1 AND u.deleted_at IS NULL AND u.status = 'active'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

pub async fn codes_by_ids(
    connection: &mut sqlx::PgConnection,
    permission_ids: &[i64],
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT code FROM permissions WHERE id = ANY($1::bigint[]) ORDER BY code")
        .bind(permission_ids)
        .fetch_all(connection)
        .await
}

pub async fn codes_by_role_ids(
    pool: &PgPool,
    role_ids: &[i64],
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT DISTINCT p.code
         FROM permissions p
         JOIN role_permissions rp ON rp.permission_id = p.id
         WHERE rp.role_id = ANY($1::bigint[])
         ORDER BY p.code",
    )
    .bind(role_ids)
    .fetch_all(pool)
    .await
}

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
         JOIN roles r ON r.id = rp.role_id AND r.is_active = TRUE
         JOIN user_roles ur ON ur.role_id = r.id
         JOIN users u ON u.id = ur.user_id
         WHERE ur.user_id = $1 AND u.deleted_at IS NULL AND u.status = 'active'
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
