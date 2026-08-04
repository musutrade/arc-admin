//! 用户服务：参数校验 + 业务规则（无 SQL）

use crate::error::{db_error, ApiError};
use crate::models::{
    user_response, AssignRolesRequest, CreateUserRequest, PageQuery, PageUser, UpdateUserRequest,
    UserResponse,
};
use crate::repositories;
use crate::services::auth;
use sqlx::PgPool;

pub async fn list(pool: &PgPool, query: &PageQuery) -> Result<PageUser, ApiError> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let rows = repositories::users::list(
        pool,
        query.keyword.clone(),
        query.status.clone(),
        page,
        page_size,
    )
    .await
    .map_err(db_error)?;
    let total = repositories::users::count(pool, query.keyword.clone(), query.status.clone())
        .await
        .map_err(db_error)?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let roles = repositories::users::role_names_by_user(pool, row.id)
            .await
            .map_err(db_error)?;
        items.push(user_response(row, roles));
    }
    Ok(PageUser {
        items,
        total,
        page,
        page_size,
    })
}

pub async fn get(pool: &PgPool, id: i64) -> Result<UserResponse, ApiError> {
    let row = repositories::users::find_by_id(pool, id)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    let roles = repositories::users::role_names_by_user(pool, id)
        .await
        .map_err(db_error)?;
    Ok(user_response(row, roles))
}

pub async fn create(pool: &PgPool, req: &CreateUserRequest) -> Result<UserResponse, ApiError> {
    let username = req.username.trim();
    if !(3..=64).contains(&username.len()) {
        return Err(ApiError::validation("username 长度需在 3-64 之间"));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ApiError::validation(
            "username 仅允许字母、数字、下划线、连字符",
        ));
    }
    if req.password.len() < 8 {
        return Err(ApiError::validation("password 长度不能少于 8 位"));
    }
    let display_name = req.display_name.trim();
    if display_name.is_empty() || display_name.len() > 128 {
        return Err(ApiError::validation("displayName 长度需在 1-128 之间"));
    }
    let status = req.status.clone().unwrap_or_else(|| "active".to_string());
    if !["active", "inactive", "suspended"].contains(&status.as_str()) {
        return Err(ApiError::validation(
            "status 只能是 active/inactive/suspended",
        ));
    }

    let hash = auth::hash_password(&req.password)?;
    let row = repositories::users::create(
        pool,
        username,
        &hash,
        display_name,
        req.email.clone(),
        &status,
    )
    .await
    .map_err(db_error)?;
    if let Some(role_ids) = &req.role_ids {
        repositories::users::assign_roles(pool, row.id, role_ids)
            .await
            .map_err(db_error)?;
    }
    let roles = repositories::users::role_names_by_user(pool, row.id)
        .await
        .map_err(db_error)?;
    Ok(user_response(row, roles))
}

pub async fn update(
    pool: &PgPool,
    id: i64,
    req: &UpdateUserRequest,
) -> Result<UserResponse, ApiError> {
    if let Some(status) = &req.status {
        if !["active", "inactive", "suspended"].contains(&status.as_str()) {
            return Err(ApiError::validation(
                "status 只能是 active/inactive/suspended",
            ));
        }
    }
    if let Some(password) = &req.password {
        if password.len() < 8 {
            return Err(ApiError::validation("password 长度不能少于 8 位"));
        }
        let hash = auth::hash_password(password)?;
        repositories::users::update_password(pool, id, &hash)
            .await
            .map_err(db_error)?
            .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    }
    let row = repositories::users::update_profile(
        pool,
        id,
        req.display_name.clone(),
        req.email.clone(),
        req.status.clone(),
    )
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    let roles = repositories::users::role_names_by_user(pool, id)
        .await
        .map_err(db_error)?;
    Ok(user_response(row, roles))
}

pub async fn delete(pool: &PgPool, id: i64, current_user_id: i64) -> Result<(), ApiError> {
    if id == current_user_id {
        return Err(ApiError::forbidden("不能删除当前登录账号"));
    }
    let deleted = repositories::users::soft_delete(pool, id)
        .await
        .map_err(db_error)?;
    if !deleted {
        return Err(ApiError::not_found("用户不存在"));
    }
    Ok(())
}

pub async fn assign_roles(
    pool: &PgPool,
    user_id: i64,
    req: &AssignRolesRequest,
) -> Result<(), ApiError> {
    repositories::users::find_by_id(pool, user_id)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    repositories::users::assign_roles(pool, user_id, &req.role_ids)
        .await
        .map_err(db_error)?;
    Ok(())
}
