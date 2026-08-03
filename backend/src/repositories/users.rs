//! 用户 Repository：唯一的用户数据访问层（SQL 写操作只允许出现在这里）

use crate::models::UserRow;
use sqlx::{PgPool, Row};

const USER_COLUMNS: &str =
    "id, username, password_hash, display_name, email, status, last_login_at, created_at";

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

pub async fn list(
    pool: &PgPool,
    keyword: Option<String>,
    status: Option<String>,
    page: i64,
    page_size: i64,
) -> Result<Vec<UserRow>, sqlx::Error> {
    let like = keyword.map(|k| format!("%{k}%"));
    sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLUMNS} FROM users
         WHERE deleted_at IS NULL
           AND ($1::text IS NULL OR username ILIKE $1 OR display_name ILIKE $1 OR email ILIKE $1)
           AND ($2::text IS NULL OR status = $2)
         ORDER BY id
         LIMIT $3 OFFSET $4"
    ))
    .bind(like)
    .bind(status)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(pool)
    .await
}

pub async fn count(
    pool: &PgPool,
    keyword: Option<String>,
    status: Option<String>,
) -> Result<i64, sqlx::Error> {
    let like = keyword.map(|k| format!("%{k}%"));
    let row = sqlx::query(
        "SELECT count(*) FROM users
         WHERE deleted_at IS NULL
           AND ($1::text IS NULL OR username ILIKE $1 OR display_name ILIKE $1 OR email ILIKE $1)
           AND ($2::text IS NULL OR status = $2)",
    )
    .bind(like)
    .bind(status)
    .fetch_one(pool)
    .await?;
    row.try_get::<i64, _>(0)
}

pub async fn create(
    pool: &PgPool,
    username: &str,
    password_hash: &str,
    display_name: &str,
    email: Option<String>,
    status: &str,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "INSERT INTO users (username, password_hash, display_name, email, status)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING {USER_COLUMNS}"
    ))
    .bind(username)
    .bind(password_hash)
    .bind(display_name)
    .bind(email)
    .bind(status)
    .fetch_one(pool)
    .await
}

pub async fn update_profile(
    pool: &PgPool,
    id: i64,
    display_name: Option<String>,
    email: Option<String>,
    status: Option<String>,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "UPDATE users
         SET display_name = COALESCE($2, display_name),
             email = COALESCE($3, email),
             status = COALESCE($4, status),
             updated_at = now()
         WHERE id = $1 AND deleted_at IS NULL
         RETURNING {USER_COLUMNS}"
    ))
    .bind(id)
    .bind(display_name)
    .bind(email)
    .bind(status)
    .fetch_optional(pool)
    .await
}

pub async fn update_password(
    pool: &PgPool,
    id: i64,
    password_hash: &str,
) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as::<_, UserRow>(&format!(
        "UPDATE users
         SET password_hash = $2, updated_at = now()
         WHERE id = $1 AND deleted_at IS NULL
         RETURNING {USER_COLUMNS}"
    ))
    .bind(id)
    .bind(password_hash)
    .fetch_optional(pool)
    .await
}

pub async fn soft_delete(pool: &PgPool, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE users SET deleted_at = now(), updated_at = now()
         WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_last_login(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
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

pub async fn assign_roles(
    pool: &PgPool,
    user_id: i64,
    role_ids: &[i64],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    for role_id in role_ids {
        sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)")
            .bind(user_id)
            .bind(role_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}
