//! 用户 Repository：唯一的用户数据访问层（SQL 写操作只允许出现在这里）

use crate::access::ActorContext;
use crate::models::{UserRow, UserWithRolesRow};
use sqlx::{PgConnection, PgPool, Row};

const USER_COLUMNS: &str =
    "id, username, password_hash, display_name, email, status, organization_id, department_id, token_version, last_login_at, created_at";

pub async fn find_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLUMNS} FROM users WHERE username = $1 AND deleted_at IS NULL"
    ))
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLUMNS} FROM users WHERE id = $1 AND deleted_at IS NULL"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_id_for_actor(
    pool: &PgPool,
    actor: &ActorContext,
    id: i64,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "WITH RECURSIVE visible_departments AS (
             SELECT d.id
             FROM departments d
             WHERE d.id = $4 AND d.organization_id = $2
             UNION
             SELECT child.id
             FROM departments child
             JOIN visible_departments parent ON child.parent_id = parent.id
             WHERE child.organization_id = $2
         )
         SELECT {USER_COLUMNS}
         FROM users
         WHERE id = $1 AND deleted_at IS NULL
           AND (
               $5 = 'all'
               OR organization_id = $2 AND (
                   $5 = 'organization'
                   OR $5 = 'self' AND id = $3
                   OR $5 = 'department' AND department_id = $4
                   OR $5 = 'department_and_children'
                      AND department_id IN (SELECT id FROM visible_departments)
               )
           )"
    ))
    .bind(id)
    .bind(actor.organization_id)
    .bind(actor.user_id)
    .bind(actor.department_id)
    .bind(actor.data_scope.as_str())
    .fetch_optional(pool)
    .await
}

pub async fn list(
    pool: &PgPool,
    actor: &ActorContext,
    keyword: Option<String>,
    status: Option<String>,
    page: i64,
    page_size: i64,
) -> Result<Vec<UserWithRolesRow>, sqlx::Error> {
    let like = keyword.map(|k| format!("%{k}%"));
    sqlx::query_as::<_, UserWithRolesRow>(
        "WITH RECURSIVE visible_departments AS (
             SELECT d.id
             FROM departments d
             WHERE d.id = $4 AND d.organization_id = $2
             UNION
             SELECT child.id
             FROM departments child
             JOIN visible_departments parent ON child.parent_id = parent.id
             WHERE child.organization_id = $2
         )
         SELECT u.id, u.username, u.display_name, u.email, u.status,
                u.last_login_at, u.created_at,
                COALESCE(
                    array_agg(DISTINCT r.name ORDER BY r.name) FILTER (WHERE r.id IS NOT NULL),
                    ARRAY[]::text[]
                ) AS roles
         FROM users u
         LEFT JOIN user_roles ur ON ur.user_id = u.id
         LEFT JOIN roles r ON r.id = ur.role_id AND r.is_active = TRUE
         WHERE u.deleted_at IS NULL
           AND (
               $1 = 'all'
               OR u.organization_id = $2 AND (
                   $1 = 'organization'
                   OR $1 = 'self' AND u.id = $3
                   OR $1 = 'department' AND u.department_id = $4
                   OR $1 = 'department_and_children'
                      AND u.department_id IN (SELECT id FROM visible_departments)
               )
           )
           AND ($5::text IS NULL OR u.username ILIKE $5 OR u.display_name ILIKE $5 OR u.email ILIKE $5)
           AND ($6::text IS NULL OR u.status = $6)
         GROUP BY u.id
         ORDER BY u.id
         LIMIT $7 OFFSET $8"
    )
    .bind(actor.data_scope.as_str())
    .bind(actor.organization_id)
    .bind(actor.user_id)
    .bind(actor.department_id)
    .bind(like)
    .bind(status)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(pool)
    .await
}

pub async fn count(
    pool: &PgPool,
    actor: &ActorContext,
    keyword: Option<String>,
    status: Option<String>,
) -> Result<i64, sqlx::Error> {
    let like = keyword.map(|k| format!("%{k}%"));
    let row = sqlx::query(
        "WITH RECURSIVE visible_departments AS (
             SELECT d.id
             FROM departments d
             WHERE d.id = $4 AND d.organization_id = $2
             UNION
             SELECT child.id
             FROM departments child
             JOIN visible_departments parent ON child.parent_id = parent.id
             WHERE child.organization_id = $2
         )
         SELECT count(*) FROM users
         WHERE deleted_at IS NULL
           AND (
               $1 = 'all'
               OR organization_id = $2 AND (
                   $1 = 'organization'
                   OR $1 = 'self' AND id = $3
                   OR $1 = 'department' AND department_id = $4
                   OR $1 = 'department_and_children'
                      AND department_id IN (SELECT id FROM visible_departments)
               )
           )
           AND ($5::text IS NULL OR username ILIKE $5 OR display_name ILIKE $5 OR email ILIKE $5)
           AND ($6::text IS NULL OR status = $6)",
    )
    .bind(actor.data_scope.as_str())
    .bind(actor.organization_id)
    .bind(actor.user_id)
    .bind(actor.department_id)
    .bind(like)
    .bind(status)
    .fetch_one(pool)
    .await?;
    row.try_get::<i64, _>(0)
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    connection: &mut PgConnection,
    username: &str,
    password_hash: &str,
    display_name: &str,
    email: Option<String>,
    status: &str,
    organization_id: i64,
    department_id: Option<i64>,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "INSERT INTO users (
             username, password_hash, display_name, email, status,
             organization_id, department_id
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING {USER_COLUMNS}"
    ))
    .bind(username)
    .bind(password_hash)
    .bind(display_name)
    .bind(email)
    .bind(status)
    .bind(organization_id)
    .bind(department_id)
    .fetch_one(&mut *connection)
    .await
}

pub async fn activate_bootstrap_account(
    connection: &mut PgConnection,
    id: i64,
    password_hash: &str,
    display_name: &str,
    email: Option<String>,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "UPDATE users
         SET password_hash = $2,
             display_name = $3,
             email = $4,
             status = 'active',
             token_version = token_version + 1,
             updated_at = now()
         WHERE id = $1 AND deleted_at IS NULL
         RETURNING {USER_COLUMNS}"
    ))
    .bind(id)
    .bind(password_hash)
    .bind(display_name)
    .bind(email)
    .fetch_one(&mut *connection)
    .await
}

pub async fn update_profile(
    connection: &mut PgConnection,
    id: i64,
    display_name: Option<String>,
    email_is_set: bool,
    email: Option<String>,
    status: Option<String>,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "UPDATE users
         SET display_name = COALESCE($2, display_name),
             email = CASE WHEN $3 THEN $4 ELSE email END,
             status = COALESCE($5, status),
             updated_at = now()
         WHERE id = $1 AND deleted_at IS NULL
         RETURNING {USER_COLUMNS}"
    ))
    .bind(id)
    .bind(display_name)
    .bind(email_is_set)
    .bind(email)
    .bind(status)
    .fetch_optional(&mut *connection)
    .await
}

pub async fn update_password(
    connection: &mut PgConnection,
    id: i64,
    password_hash: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "UPDATE users
         SET password_hash = $2, token_version = token_version + 1, updated_at = now()
         WHERE id = $1 AND deleted_at IS NULL
         RETURNING {USER_COLUMNS}"
    ))
    .bind(id)
    .bind(password_hash)
    .fetch_optional(&mut *connection)
    .await
}

pub async fn soft_delete(connection: &mut PgConnection, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE users SET deleted_at = now(), updated_at = now()
         WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(&mut *connection)
    .await?;
    if result.rows_affected() > 0 {
        sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
            .bind(id)
            .execute(&mut *connection)
            .await?;
    }
    Ok(result.rows_affected() > 0)
}

pub async fn super_admin_guard_state(
    connection: &mut PgConnection,
    user_id: i64,
) -> Result<(bool, i64), sqlx::Error> {
    let role_id =
        sqlx::query_scalar::<_, i64>("SELECT id FROM roles WHERE code = 'super_admin' FOR UPDATE")
            .fetch_optional(&mut *connection)
            .await?;
    let Some(role_id) = role_id else {
        return Ok((false, 0));
    };
    sqlx::query_as::<_, (bool, i64)>(
        "SELECT
            EXISTS(
                SELECT 1 FROM users u
                JOIN user_roles ur ON ur.user_id = u.id
                WHERE u.id = $1 AND ur.role_id = $2
                  AND u.deleted_at IS NULL AND u.status = 'active'
            ),
            (SELECT count(DISTINCT u.id)
             FROM users u
             JOIN user_roles ur ON ur.user_id = u.id
             WHERE ur.role_id = $2 AND u.deleted_at IS NULL AND u.status = 'active')",
    )
    .bind(user_id)
    .bind(role_id)
    .fetch_one(&mut *connection)
    .await
}

pub async fn update_last_login(connection: &mut PgConnection, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(id)
        .execute(connection)
        .await?;
    Ok(())
}

pub async fn role_names_by_user(pool: &PgPool, user_id: i64) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT r.name FROM roles r
         JOIN user_roles ur ON ur.role_id = r.id
         WHERE ur.user_id = $1
         ORDER BY r.id",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn role_ids_by_user(pool: &PgPool, user_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar("SELECT role_id FROM user_roles WHERE user_id = $1 ORDER BY role_id")
        .bind(user_id)
        .fetch_all(pool)
        .await
}

pub async fn assign_roles(
    connection: &mut PgConnection,
    user_id: i64,
    role_ids: &[i64],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *connection)
        .await?;
    for role_id in role_ids {
        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
            .bind(user_id)
            .bind(role_id)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}
