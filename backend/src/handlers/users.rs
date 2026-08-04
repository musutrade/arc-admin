//! 用户 Handler：列表 / 详情 / 创建 / 更新 / 软删除 / 分配角色

use crate::auth::{RequirePermission, UserDeactivate, UserRead, UserWrite};
use crate::error::ApiError;
use crate::models::{
    AssignRolesRequest, CreateUserRequest, PageQuery, PageUser, UpdateUserRequest, UserResponse,
};
use crate::services;
use crate::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

pub async fn list(
    State(state): State<AppState>,
    _auth: RequirePermission<UserRead>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PageUser>, ApiError> {
    services::users::list(&state.pool, &query).await.map(Json)
}

pub async fn get(
    State(state): State<AppState>,
    _auth: RequirePermission<UserRead>,
    Path(id): Path<i64>,
) -> Result<Json<UserResponse>, ApiError> {
    services::users::get(&state.pool, id).await.map(Json)
}

pub async fn create(
    State(state): State<AppState>,
    _auth: RequirePermission<UserWrite>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), ApiError> {
    services::users::create(&state.pool, &req)
        .await
        .map(|user| (StatusCode::CREATED, Json(user)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: RequirePermission<UserWrite>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    if req.password.is_some() {
        auth.require("user:admin:reset_password")?;
    }
    if req
        .status
        .as_deref()
        .is_some_and(|status| status != "active")
    {
        auth.require("user:admin:deactivate")?;
    }
    services::users::update(&state.pool, id, &req)
        .await
        .map(Json)
}

pub async fn delete(
    State(state): State<AppState>,
    auth: RequirePermission<UserDeactivate>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    services::users::delete(&state.pool, id, auth.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn assign_roles(
    State(state): State<AppState>,
    _auth: RequirePermission<UserWrite>,
    Path(id): Path<i64>,
    Json(req): Json<AssignRolesRequest>,
) -> Result<StatusCode, ApiError> {
    services::users::assign_roles(&state.pool, id, &req).await?;
    Ok(StatusCode::NO_CONTENT)
}
