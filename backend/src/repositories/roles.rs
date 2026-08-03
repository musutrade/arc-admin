//! 角色 Repository：唯一的角色数据访问层（SQL 写操作只允许出现在这里）

use crate::models::RoleRow;
use sqlx::{PgPool, Row};

const ROLE_SELECT: &str = "SELECT r.id, r.code, r.name, r.category, r.icon, r.color, \
    r.description, r.is_active, \
    (SELECT count(*) FROM user_roles ur WHERE ur.role_id = r.id) AS members \
    FROM roles r";

pub async fn list_all(pool: &PgPool) -> Result<Vec<RoleRow>, sqlx::Error> {
    sqlx::query_as::<_, RoleRow>(&format!("{ROLE_SELECT} ORDER BY r.id"))
        .fetch_all(pool)
        .await
}

pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<RoleRow>, sqlx::Error> {
    sqlx::query_as::<_, RoleRow>(&format!("{ROLE_SELECT} WHERE r.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create(
    pool: &PgPool,
    code: &str,
    name: &str,
    category: &str,
    icon: Option<String>,
    color: Option<String>,
    description: Option<String>,
) -> Result<i64, sqlx::Error> {
    let row = sqlx::query(
        "INSERT INTO roles (code, name, category, icon, color, description)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(code)
    .bind(name)
    .bind(category)
    .bind(icon)
    .bind(color)
    .bind(description)
    .fetch_one(pool)
    .await?;
    row.try_get::<i64, _>(0)
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    pool: &PgPool,
    id: i64,
    name: Option<String>,
    category: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    description: Option<String>,
    is_active: Option<bool>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE roles
         SET name = COALESCE($2, name),
             category = COALESCE($3, category),
             icon = COALESCE($4, icon),
             color = COALESCE($5, color),
             description = COALESCE($6, description),
             is_active = COALESCE($7, is_active),
             updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(name)
    .bind(category)
    .bind(icon)
    .bind(color)
    .bind(description)
    .bind(is_active)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM roles WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn permission_ids_by_role(pool: &PgPool, role_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT permission_id FROM role_permissions WHERE role_id = $1 ORDER BY permission_id",
    )
    .bind(role_id)
    .fetch_all(pool)
    .await
}

pub async fn permission_group_ids_by_role(
    pool: &PgPool,
    role_id: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT DISTINCT pg.id FROM permission_groups pg
         JOIN permissions p ON p.group_id = pg.id
         JOIN role_permissions rp ON rp.permission_id = p.id
         WHERE rp.role_id = $1
         ORDER BY pg.id",
    )
    .bind(role_id)
    .fetch_all(pool)
    .await
}

pub async fn assign_permissions(
    pool: &PgPool,
    role_id: i64,
    permission_ids: &[i64],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
        .bind(role_id)
        .execute(&mut *tx)
        .await?;
    for permission_id in permission_ids {
        sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)")
            .bind(role_id)
            .bind(permission_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}
