//! Authentication service: password verification, session lifecycle, throttling, and auditing.

use crate::auth::{self, AuthSessionConfig};
use crate::error::{db_error, ApiError};
use crate::models::{
    user_response, ChangePasswordRequest, LoginRequest, LoginResponse, PermissionCodes,
    UserResponse, UserRow,
};
use crate::repositories;
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;

pub const MIN_PASSWORD_LENGTH: usize = 12;
pub const MAX_PASSWORD_LENGTH: usize = 128;
const BOOTSTRAP_PASSWORD_LENGTH: usize = 16;
const DUMMY_PASSWORD_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$pDDhKh46fVQNqRy3OeXTTw$+5qvGkvmKsilvsRWsskXT4k6fmmE4q35ntz6ME1UNBE";

pub struct LoginOutcome {
    pub response: LoginResponse,
    pub session_token: String,
    pub csrf_token: String,
    pub persistent: bool,
    pub ttl_secs: i64,
}

pub async fn login(
    pool: &PgPool,
    config: &AuthSessionConfig,
    req: &LoginRequest,
) -> Result<LoginOutcome, ApiError> {
    let username = req.username.trim();
    let identifier_hash = auth::token_hash(&username.to_ascii_lowercase());
    if let Some(locked_until) = repositories::auth_sessions::locked_until(pool, &identifier_hash)
        .await
        .map_err(db_error)?
    {
        let retry_after = (locked_until - Utc::now()).num_seconds().max(1) as u64;
        return Err(ApiError::rate_limited(retry_after));
    }

    let row = repositories::users::find_by_username(pool, username)
        .await
        .map_err(db_error)?;
    let password_hash = row
        .as_ref()
        .map(|user| user.password_hash.as_str())
        .unwrap_or(DUMMY_PASSWORD_HASH);
    let password_valid = req.password.chars().count() <= MAX_PASSWORD_LENGTH
        && PasswordHash::new(password_hash).ok().is_some_and(|parsed| {
            Argon2::default()
                .verify_password(req.password.as_bytes(), &parsed)
                .is_ok()
        });
    let authenticated = row
        .as_ref()
        .is_some_and(|user| user.status == "active" && password_valid);
    if !authenticated {
        let locked_until = record_failed_login(
            pool,
            config,
            &identifier_hash,
            row.as_ref().map(|user| user.id),
        )
        .await?;
        if let Some(locked_until) = locked_until {
            let retry_after = (locked_until - Utc::now()).num_seconds().max(1) as u64;
            return Err(ApiError::rate_limited(retry_after));
        }
        return Err(ApiError::unauthorized());
    }
    let row = row.expect("authenticated login always has a user row");

    create_login_session(pool, config, req.remember, identifier_hash, row).await
}

async fn create_login_session(
    pool: &PgPool,
    config: &AuthSessionConfig,
    persistent: bool,
    identifier_hash: String,
    row: UserRow,
) -> Result<LoginOutcome, ApiError> {
    let (ttl_secs, idle_timeout_secs) = if persistent {
        (
            config.persistent_session_ttl_secs,
            config.persistent_session_idle_timeout_secs,
        )
    } else {
        (config.session_ttl_secs, config.session_idle_timeout_secs)
    };
    let expires_at = Utc::now() + Duration::seconds(ttl_secs);
    let session_token = auth::random_token();
    let csrf_token = auth::random_token();
    let mut transaction = pool.begin().await.map_err(db_error)?;
    repositories::users::update_last_login(&mut transaction, row.id)
        .await
        .map_err(db_error)?;
    repositories::auth_sessions::clear_login_failures(&mut transaction, &identifier_hash)
        .await
        .map_err(db_error)?;
    let session_id = repositories::auth_sessions::create(
        &mut transaction,
        row.id,
        &auth::token_hash(&session_token),
        &auth::token_hash(&csrf_token),
        row.token_version,
        idle_timeout_secs,
        persistent,
        expires_at,
    )
    .await
    .map_err(db_error)?;
    let revoked_sessions = repositories::auth_sessions::enforce_user_limit(
        &mut transaction,
        row.id,
        config.max_sessions_per_user,
    )
    .await
    .map_err(db_error)?;
    repositories::auth_sessions::prune(&mut transaction)
        .await
        .map_err(db_error)?;
    repositories::audit_logs::record(
        &mut transaction,
        Some(row.id),
        "auth.login.success",
        "auth_session",
        Some(session_id),
        json!({
            "persistent": persistent,
            "expiresAt": expires_at,
            "revokedBySessionLimit": revoked_sessions,
        }),
    )
    .await
    .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;

    let roles = repositories::users::role_names_by_user(pool, row.id)
        .await
        .map_err(db_error)?;
    Ok(LoginOutcome {
        response: LoginResponse {
            expires_at,
            user: user_response(row, roles),
        },
        session_token,
        csrf_token,
        persistent,
        ttl_secs,
    })
}

async fn record_failed_login(
    pool: &PgPool,
    config: &AuthSessionConfig,
    identifier_hash: &str,
    user_id: Option<i64>,
) -> Result<Option<chrono::DateTime<Utc>>, ApiError> {
    let mut transaction = pool.begin().await.map_err(db_error)?;
    let locked_until = repositories::auth_sessions::record_login_failure(
        &mut transaction,
        identifier_hash,
        config.login_max_failures,
        config.login_failure_window_secs,
        config.login_lockout_secs,
    )
    .await
    .map_err(db_error)?;
    repositories::audit_logs::record(
        &mut transaction,
        None,
        "auth.login.failure",
        "user",
        user_id,
        json!({
            "identifierFingerprint": &identifier_hash[..12],
            "locked": locked_until.is_some(),
        }),
    )
    .await
    .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    Ok(locked_until)
}

pub async fn logout(pool: &PgPool, user_id: i64, session_id: i64) -> Result<(), ApiError> {
    let mut transaction = pool.begin().await.map_err(db_error)?;
    repositories::auth_sessions::revoke(&mut transaction, session_id, user_id)
        .await
        .map_err(db_error)?;
    repositories::audit_logs::record(
        &mut transaction,
        Some(user_id),
        "auth.logout",
        "auth_session",
        Some(session_id),
        json!({}),
    )
    .await
    .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    Ok(())
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
    validate_password(&req.new_password)?;
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
    let revoked_sessions =
        repositories::auth_sessions::revoke_all_for_user(&mut transaction, user_id)
            .await
            .map_err(db_error)?;
    repositories::audit_logs::record(
        &mut transaction,
        Some(user_id),
        "user.password.change",
        "user",
        Some(user_id),
        json!({
            "revokedExistingSessions": true,
            "revokedSessionCount": revoked_sessions,
        }),
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
    validate_password(password)?;
    if password.chars().count() < BOOTSTRAP_PASSWORD_LENGTH {
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
    repositories::auth_sessions::revoke_all_for_user(&mut transaction, row.id)
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

pub fn validate_password(password: &str) -> Result<(), ApiError> {
    let length = password.chars().count();
    if !(MIN_PASSWORD_LENGTH..=MAX_PASSWORD_LENGTH).contains(&length) {
        return Err(ApiError::validation("密码长度需在 12-128 个字符之间"));
    }
    Ok(())
}

/// Argon2 password hash in PHC format with a cryptographically random salt.
pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| ApiError::internal(error.to_string()))
}
