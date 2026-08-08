//! 认证服务：登录校验（argon2）、JWT 签发、当前用户与权限码

use crate::auth::Claims;
use crate::error::{db_error, ApiError};
use crate::models::{
    user_response, ChangePasswordRequest, LoginRequest, LoginResponse, PermissionCodes,
    UserResponse,
};
use crate::repositories;
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::json;
use sqlx::PgPool;

pub async fn login(
    pool: &PgPool,
    jwt_secret: &str,
    token_ttl_secs: i64,
    req: &LoginRequest,
) -> Result<LoginResponse, ApiError> {
    let row = repositories::users::find_by_username(pool, &req.username)
        .await
        .map_err(db_error)?
        .ok_or_else(ApiError::unauthorized)?;
    if row.status != "active" {
        return Err(ApiError::unauthorized());
    }

    let parsed = PasswordHash::new(&row.password_hash).map_err(|_| ApiError::unauthorized())?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed)
        .map_err(|_| ApiError::unauthorized())?;

    repositories::users::update_last_login(pool, row.id)
        .await
        .map_err(db_error)?;
    let roles = repositories::users::role_names_by_user(pool, row.id)
        .await
        .map_err(db_error)?;

    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: row.id.to_string(),
        iat: now as usize,
        exp: (now + token_ttl_secs) as usize,
        ver: row.token_version,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(ApiError::internal)?;

    Ok(LoginResponse {
        access_token: token,
        token_type: "Bearer".to_string(),
        expires_in: token_ttl_secs,
        user: user_response(row, roles),
    })
}

pub async fn me(pool: &PgPool, user_id: i64) -> Result<UserResponse, ApiError> {
    let row = repositories::users::find_by_id(pool, user_id)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    let roles = repositories::users::role_names_by_user(pool, user_id)
        .await
        .map_err(db_error)?;
    Ok(user_response(row, roles))
}

pub async fn change_password(
    pool: &PgPool,
    user_id: i64,
    req: &ChangePasswordRequest,
) -> Result<(), ApiError> {
    if req.current_password.is_empty() {
        return Err(ApiError::validation("当前密码不能为空"));
    }
    if req.new_password.len() < 8 {
        return Err(ApiError::validation("新密码长度不能少于 8 位"));
    }
    if req.current_password == req.new_password {
        return Err(ApiError::validation("新密码不能与当前密码相同"));
    }

    let row = repositories::users::find_by_id(pool, user_id)
        .await
        .map_err(db_error)?
        .ok_or_else(ApiError::unauthorized)?;
    let parsed = PasswordHash::new(&row.password_hash)
        .map_err(|error| ApiError::internal(format!("invalid stored password hash: {error}")))?;
    Argon2::default()
        .verify_password(req.current_password.as_bytes(), &parsed)
        .map_err(|_| ApiError::validation("当前密码不正确"))?;

    let password_hash = hash_password(&req.new_password)?;
    let mut transaction = pool.begin().await.map_err(db_error)?;
    repositories::users::update_password(&mut transaction, user_id, &password_hash)
        .await
        .map_err(db_error)?
        .ok_or_else(ApiError::unauthorized)?;
    repositories::audit_logs::record(
        &mut transaction,
        Some(user_id),
        "user.password.change",
        "user",
        Some(user_id),
        json!({ "revokedExistingSessions": true }),
    )
    .await
    .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    Ok(())
}

pub async fn permission_codes(pool: &PgPool, user_id: i64) -> Result<PermissionCodes, ApiError> {
    let codes = repositories::permissions::permission_codes_by_user(pool, user_id)
        .await
        .map_err(db_error)?;
    Ok(PermissionCodes { codes })
}

pub async fn bootstrap_super_admin(
    pool: &PgPool,
    username: &str,
    password: &str,
    display_name: &str,
    email: Option<String>,
) -> Result<UserResponse, ApiError> {
    let username = username.trim();
    if !(3..=64).contains(&username.len())
        || !username.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        })
    {
        return Err(ApiError::validation(
            "管理员用户名需为 3-64 位字母、数字、下划线或连字符",
        ));
    }
    if password.len() < 16 {
        return Err(ApiError::validation("引导管理员密码不能少于 16 位"));
    }
    let display_name = display_name.trim();
    if display_name.is_empty() || display_name.len() > 128 {
        return Err(ApiError::validation("显示名称长度需在 1-128 个字符之间"));
    }

    let role_id = repositories::roles::id_by_code(pool, "super_admin")
        .await
        .map_err(db_error)?
        .ok_or_else(|| ApiError::internal("缺少内置超级管理员角色"))?;
    let existing = repositories::users::find_by_username(pool, username)
        .await
        .map_err(db_error)?;
    let password_hash = hash_password(password)?;
    let mut transaction = pool.begin().await.map_err(db_error)?;
    let row = if let Some(existing) = existing {
        repositories::users::activate_bootstrap_account(
            &mut transaction,
            existing.id,
            &password_hash,
            display_name,
            email,
        )
        .await
        .map_err(db_error)?
    } else {
        repositories::users::create(
            &mut transaction,
            username,
            &password_hash,
            display_name,
            email,
            "active",
        )
        .await
        .map_err(db_error)?
    };
    repositories::users::assign_roles(&mut transaction, row.id, &[role_id])
        .await
        .map_err(db_error)?;
    repositories::audit_logs::record(
        &mut transaction,
        None,
        "user.bootstrap_super_admin",
        "user",
        Some(row.id),
        json!({ "username": username, "roleId": role_id }),
    )
    .await
    .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    let roles = repositories::users::role_names_by_user(pool, row.id)
        .await
        .map_err(db_error)?;
    Ok(user_response(row, roles))
}

/// argon2 密码哈希（PHC 字符串，含随机盐）
pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| ApiError::internal(e.to_string()))
}
