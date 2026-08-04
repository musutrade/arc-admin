//! 认证服务：登录校验（argon2）、JWT 签发、当前用户与权限码

use crate::auth::Claims;
use crate::error::{db_error, ApiError};
use crate::models::{user_response, LoginRequest, LoginResponse, PermissionCodes, UserResponse};
use crate::repositories;
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
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

pub async fn permission_codes(pool: &PgPool, user_id: i64) -> Result<PermissionCodes, ApiError> {
    let codes = repositories::permissions::permission_codes_by_user(pool, user_id)
        .await
        .map_err(db_error)?;
    Ok(PermissionCodes { codes })
}

/// argon2 密码哈希（PHC 字符串，含随机盐）
pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| ApiError::internal(e.to_string()))
}
