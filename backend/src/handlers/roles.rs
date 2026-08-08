//! 角色 Handler：列表 / 详情 / 创建 / 更新 / 删除 / 权限分配

use crate::auth::{RequirePermission, RolePermissionWrite, RoleRead, RoleWrite};
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
    _auth: RequirePermission<RoleRead>,
) -> Result<Json<Vec<RoleResponse>>, ApiError> {
    services::roles::list(&state.pool).await.map(Json)
}

pub async fn get(
    State(state): State<AppState>,
    _auth: RequirePermission<RoleRead>,
    Path(id): Path<i64>,
) -> Result<Json<RoleResponse>, ApiError> {
    services::roles::get(&state.pool, id).await.map(Json)
}

pub async fn create(
    State(state): State<AppState>,
    auth: RequirePermission<RoleWrite>,
    Json(req): Json<CreateRoleRequest>,
) -> Result<(StatusCode, Json<RoleResponse>), ApiError> {
    if req
        .permission_ids
        .as_ref()
        .is_some_and(|permission_ids| !permission_ids.is_empty())
    {
        auth.require("role:permissions:write")?;
    }
    services::roles::create(
        &state.pool,
        Some(auth.user_id),
        &req,
        auth.has("role:permissions:write"),
    )
    .await
    .map(|role| (StatusCode::CREATED, Json(role)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: RequirePermission<RoleWrite>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<RoleResponse>, ApiError> {
    services::roles::update(&state.pool, Some(auth.user_id), id, &req)
        .await
        .map(Json)
}

pub async fn delete(
    State(state): State<AppState>,
    auth: RequirePermission<RoleWrite>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    services::roles::delete(&state.pool, Some(auth.user_id), id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_permissions(
    State(state): State<AppState>,
    _auth: RequirePermission<RoleRead>,
    Path(id): Path<i64>,
) -> Result<Json<RolePermissions>, ApiError> {
    services::roles::get_permissions(&state.pool, id)
        .await
        .map(Json)
}

pub async fn put_permissions(
    State(state): State<AppState>,
    auth: RequirePermission<RolePermissionWrite>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateRolePermissionsRequest>,
) -> Result<StatusCode, ApiError> {
    services::roles::assign_permissions(&state.pool, Some(auth.user_id), id, &req).await?;
    Ok(StatusCode::NO_CONTENT)
}
