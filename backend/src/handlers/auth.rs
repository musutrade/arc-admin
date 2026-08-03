//! 认证 Handler：登录 / 当前用户 / 权限码

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::models::{LoginRequest, LoginResponse, PermissionCodes, UserResponse};
use crate::services;
use crate::AppState;
use axum::extract::State;
use axum::Json;

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    services::auth::login(&state.pool, &state.jwt_secret, state.token_ttl_secs, &req)
        .await
        .map(Json)
}

pub async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<UserResponse>, ApiError> {
    services::auth::me(&state.pool, auth.user_id).await.map(Json)
}

pub async fn me_permissions(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<PermissionCodes>, ApiError> {
    services::auth::permission_codes(&state.pool, auth.user_id)
        .await
        .map(Json)
}

