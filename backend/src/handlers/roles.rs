//! 角色 Handler：列表 / 详情 / 创建 / 更新 / 删除 / 权限分配

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::models::{
    CreateRoleRequest, RolePermissions, RoleResponse, UpdateRolePermissionsRequest,
    UpdateRoleRequest,
};
use crate::services;
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

pub async fn list(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<RoleResponse>>, ApiError> {
    services::roles::list(&state.pool).await.map(Json)
}

pub async fn get(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<RoleResponse>, ApiError> {
    services::roles::get(&state.pool, id).await.map(Json)
}

pub async fn create(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(req): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<RoleResponse>), ApiError> {
    services::roles::create(&state.pool, &req)
        .await
        .map(|role| (StatusCode::CREATED, Json(role)))
}

pub async fn update(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<RoleResponse>, ApiError> {
    services::roles::update(&state.pool, id, &req).await.map(Json)
}

pub async fn delete(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    services::roles::delete(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_permissions(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<RolePermissions>, ApiError> {
    services::roles::get_permissions(&state.pool, id).await.map(Json)
}

pub async fn put_permissions(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<UpdateRolePermissionsRequest>,
) -> Result<StatusCode, ApiError> {
    services::roles::assign_permissions(&state.pool, id, &req).await?;
    Ok(StatusCode::NO_CONTENT)
}

