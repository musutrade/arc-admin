//! 角色服务：参数校验 + 业务规则（无 SQL）

use crate::error::{db_error, ApiError};
use crate::models::{
    nullable_patch, CreateRoleRequest, RolePermissions, RoleResponse, RoleRow,
    RoleWithPermissionsRow, UpdateRolePermissionsRequest, UpdateRoleRequest,
};
use crate::repositories;
use sqlx::PgPool;

const ROLE_COLORS: [&str; 5] = ["primary", "warning", "success", "danger", "neutral"];

async fn to_response(pool: &PgPool, row: RoleRow) -> Result<RoleResponse, ApiError> {
    let permission_group_ids = repositories::roles::permission_group_ids_by_role(pool, row.id)
        .await
        .map_err(db_error)?;
    Ok(RoleResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        category: row.category,
        icon: row.icon,
        color: row.color,
        description: row.description,
        is_active: row.is_active,
        members: row.members,
        permission_group_ids,
    })
}

pub async fn list(pool: &PgPool) -> Result<Vec<RoleResponse>, ApiError> {
    let rows = repositories::roles::list_all(pool)
        .await
        .map_err(db_error)?;
    Ok(rows.into_iter().map(list_response).collect())
}

fn list_response(row: RoleWithPermissionsRow) -> RoleResponse {
    RoleResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        category: row.category,
        icon: row.icon,
        color: row.color,
        description: row.description,
        is_active: row.is_active,
        members: row.members,
        permission_group_ids: row.permission_group_ids,
    }
}

pub async fn get(pool: &PgPool, id: i64) -> Result<RoleResponse, ApiError> {
    let row = repositories::roles::find_by_id(pool, id)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ApiError::not_found("角色不存在"))?;
    to_response(pool, row).await
}

pub async fn create(pool: &PgPool, req: &CreateRoleRequest) -> Result<RoleResponse, ApiError> {
    let code = req.code.trim();
    let valid_code = (3..=64).contains(&code.len())
        && code.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && code
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid_code {
        return Err(ApiError::validation(
            "code 需为 3-64 位小写字母/数字/下划线，且以字母开头",
        ));
    }
    if req.name.trim().is_empty() {
        return Err(ApiError::validation("name 不能为空"));
    }
    let color = req.color.clone().unwrap_or_else(|| "neutral".to_string());
    if !ROLE_COLORS.contains(&color.as_str()) {
        return Err(ApiError::validation(
            "color 只能是 primary/warning/success/danger/neutral",
        ));
    }
    let category = req
        .category
        .clone()
        .unwrap_or_else(|| "general".to_string());

    let mut transaction = pool.begin().await.map_err(db_error)?;
    let id = repositories::roles::create(
        &mut transaction,
        code,
        req.name.trim(),
        &category,
        req.icon.clone(),
        Some(color),
        req.description.clone(),
    )
    .await
    .map_err(db_error)?;
    if let Some(permission_ids) = &req.permission_ids {
        repositories::roles::assign_permissions(&mut transaction, id, permission_ids)
            .await
            .map_err(db_error)?;
    }
    transaction.commit().await.map_err(db_error)?;
    get(pool, id).await
}

pub async fn update(
    pool: &PgPool,
    id: i64,
    req: &UpdateRoleRequest,
) -> Result<RoleResponse, ApiError> {
    let name = req.name.as_ref().map(|value| value.trim().to_string());
    if name.as_deref().is_some_and(str::is_empty) {
        return Err(ApiError::validation("name 不能为空"));
    }
    let category = req.category.as_ref().map(|value| value.trim().to_string());
    if category.as_deref().is_some_and(str::is_empty) {
        return Err(ApiError::validation("category 不能为空"));
    }
    if req.is_active == Some(false) {
        let role = repositories::roles::find_by_id(pool, id)
            .await
            .map_err(db_error)?
            .ok_or_else(|| ApiError::not_found("角色不存在"))?;
        if role.code == "super_admin" {
            return Err(ApiError::forbidden("内置角色 super_admin 不可停用"));
        }
    }
    if let Some(color) = &req.color {
        if !ROLE_COLORS.contains(&color.as_str()) {
            return Err(ApiError::validation(
                "color 只能是 primary/warning/success/danger/neutral",
            ));
        }
    }
    let (icon_is_set, icon) = nullable_patch(&req.icon);
    let (description_is_set, description) = nullable_patch(&req.description);
    let updated = repositories::roles::update(
        pool,
        id,
        name,
        category,
        icon_is_set,
        icon,
        req.color.clone(),
        description_is_set,
        description,
        req.is_active,
    )
    .await
    .map_err(db_error)?;
    if !updated {
        return Err(ApiError::not_found("角色不存在"));
    }
    get(pool, id).await
}

pub async fn delete(pool: &PgPool, id: i64) -> Result<(), ApiError> {
    if let Some(row) = repositories::roles::find_by_id(pool, id)
        .await
        .map_err(db_error)?
    {
        if row.code == "super_admin" {
            return Err(ApiError::forbidden("内置角色 super_admin 不可删除"));
        }
    }
    let deleted = repositories::roles::delete(pool, id)
        .await
        .map_err(db_error)?;
    if !deleted {
        return Err(ApiError::not_found("角色不存在"));
    }
    Ok(())
}

pub async fn get_permissions(pool: &PgPool, id: i64) -> Result<RolePermissions, ApiError> {
    repositories::roles::find_by_id(pool, id)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ApiError::not_found("角色不存在"))?;
    let permission_ids = repositories::roles::permission_ids_by_role(pool, id)
        .await
        .map_err(db_error)?;
    Ok(RolePermissions { permission_ids })
}

pub async fn assign_permissions(
    pool: &PgPool,
    id: i64,
    req: &UpdateRolePermissionsRequest,
) -> Result<(), ApiError> {
    let role = repositories::roles::find_by_id(pool, id)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ApiError::not_found("角色不存在"))?;
    if role.code == "super_admin" {
        return Err(ApiError::forbidden("内置角色 super_admin 的权限不可修改"));
    }
    let mut transaction = pool.begin().await.map_err(db_error)?;
    repositories::roles::assign_permissions(&mut transaction, id, &req.permission_ids)
        .await
        .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    Ok(())
}
