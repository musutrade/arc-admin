//! 用户服务：参数校验 + 业务规则（无 SQL）

use crate::error::{db_error, ApiError};
use crate::models::{
    nullable_patch, user_response, user_with_roles_response, AssignRolesRequest, CreateUserRequest,
    PageQuery, PageUser, UpdateUserRequest, UserResponse,
};
use crate::repositories;
use crate::services::auth;
use serde_json::json;
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

    let items = rows.into_iter().map(user_with_roles_response).collect();
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

pub async fn create(
    pool: &PgPool,
    actor_user_id: Option<i64>,
    req: &CreateUserRequest,
    can_grant_super_admin: bool,
) -> Result<UserResponse, ApiError> {
    let username = req.username.trim();
    if !(3..=64).contains(&username.len()) {
        return Err(ApiError::validation("用户名长度需在 3-64 个字符之间"));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ApiError::validation(
            "用户名仅允许字母、数字、下划线或连字符",
        ));
    }
    auth::validate_password(&req.password)?;
    let display_name = req.display_name.trim();
    if display_name.is_empty() || display_name.len() > 128 {
        return Err(ApiError::validation("显示名称长度需在 1-128 个字符之间"));
    }
    let status = req.status.clone().unwrap_or_else(|| "active".to_string());
    if !["active", "inactive", "suspended"].contains(&status.as_str()) {
        return Err(ApiError::validation("状态只能为启用、停用或已暂停"));
    }

    let hash = auth::hash_password(&req.password)?;
    if let Some(role_ids) = &req.role_ids {
        validate_role_grant_scope(pool, actor_user_id, role_ids, can_grant_super_admin).await?;
    }
    let mut transaction = pool.begin().await.map_err(db_error)?;
    let row = repositories::users::create(
        &mut transaction,
        username,
        &hash,
        display_name,
        req.email.clone(),
        &status,
    )
    .await
    .map_err(db_error)?;
    if let Some(role_ids) = &req.role_ids {
        repositories::users::assign_roles(&mut transaction, row.id, role_ids)
            .await
            .map_err(db_error)?;
    }
    repositories::audit_logs::record(
        &mut transaction,
        actor_user_id,
        "user.create",
        "user",
        Some(row.id),
        json!({
            "username": username,
            "status": status,
            "roleIds": req.role_ids.as_deref().unwrap_or_default(),
        }),
    )
    .await
    .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    let roles = repositories::users::role_names_by_user(pool, row.id)
        .await
        .map_err(db_error)?;
    Ok(user_response(row, roles))
}

pub async fn update(
    pool: &PgPool,
    id: i64,
    actor_user_id: Option<i64>,
    req: &UpdateUserRequest,
    can_grant_super_admin: bool,
) -> Result<UserResponse, ApiError> {
    let display_name = req
        .display_name
        .as_ref()
        .map(|value| value.trim().to_string());
    if display_name
        .as_deref()
        .is_some_and(|value| value.is_empty() || value.len() > 128)
    {
        return Err(ApiError::validation("显示名称长度需在 1-128 个字符之间"));
    }
    if let Some(status) = &req.status {
        if !["active", "inactive", "suspended"].contains(&status.as_str()) {
            return Err(ApiError::validation("状态只能为启用、停用或已暂停"));
        }
        if status != "active" && actor_user_id == Some(id) {
            return Err(ApiError::forbidden("不能停用当前登录账号"));
        }
        if status == "active" {
            let role_ids = repositories::users::role_ids_by_user(pool, id)
                .await
                .map_err(db_error)?;
            validate_role_grant_scope(pool, actor_user_id, &role_ids, can_grant_super_admin)
                .await?;
        }
    }
    let password_hash = if let Some(password) = &req.password {
        auth::validate_password(password)?;
        Some(auth::hash_password(password)?)
    } else {
        None
    };
    let (email_is_set, email) = nullable_patch(&req.email);
    let email = email.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    });

    let mut transaction = pool.begin().await.map_err(db_error)?;
    if req
        .status
        .as_deref()
        .is_some_and(|status| status != "active")
    {
        protect_last_super_admin(&mut transaction, id).await?;
    }
    if let Some(hash) = password_hash {
        repositories::users::update_password(&mut transaction, id, &hash)
            .await
            .map_err(db_error)?
            .ok_or_else(|| ApiError::not_found("用户不存在"))?;
        repositories::auth_sessions::revoke_all_for_user(&mut transaction, id)
            .await
            .map_err(db_error)?;
    }
    let row = repositories::users::update_profile(
        &mut transaction,
        id,
        display_name,
        email_is_set,
        email,
        req.status.clone(),
    )
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    if req
        .status
        .as_deref()
        .is_some_and(|status| status != "active")
    {
        repositories::auth_sessions::revoke_all_for_user(&mut transaction, id)
            .await
            .map_err(db_error)?;
    }
    repositories::audit_logs::record(
        &mut transaction,
        actor_user_id,
        "user.update",
        "user",
        Some(id),
        json!({
            "displayNameChanged": req.display_name.is_some(),
            "emailChanged": !matches!(req.email, crate::models::NullablePatch::Missing),
            "status": req.status,
            "passwordReset": req.password.is_some(),
        }),
    )
    .await
    .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    let roles = repositories::users::role_names_by_user(pool, id)
        .await
        .map_err(db_error)?;
    Ok(user_response(row, roles))
}

pub async fn delete(pool: &PgPool, id: i64, actor_user_id: Option<i64>) -> Result<(), ApiError> {
    if actor_user_id == Some(id) {
        return Err(ApiError::forbidden("不能删除当前登录账号"));
    }
    let mut transaction = pool.begin().await.map_err(db_error)?;
    protect_last_super_admin(&mut transaction, id).await?;
    let previous_role_ids = repositories::users::role_ids_by_user(pool, id)
        .await
        .map_err(db_error)?;
    let deleted = repositories::users::soft_delete(&mut transaction, id)
        .await
        .map_err(db_error)?;
    if !deleted {
        return Err(ApiError::not_found("用户不存在"));
    }
    repositories::audit_logs::record(
        &mut transaction,
        actor_user_id,
        "user.delete",
        "user",
        Some(id),
        json!({ "previousRoleIds": previous_role_ids }),
    )
    .await
    .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    Ok(())
}

pub async fn assign_roles(
    pool: &PgPool,
    actor_user_id: Option<i64>,
    user_id: i64,
    req: &AssignRolesRequest,
    can_grant_super_admin: bool,
) -> Result<(), ApiError> {
    repositories::users::find_by_id(pool, user_id)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ApiError::not_found("用户不存在"))?;
    let super_admin_role_id = repositories::roles::id_by_code(pool, "super_admin")
        .await
        .map_err(db_error)?;
    validate_role_grant_scope(pool, actor_user_id, &req.role_ids, can_grant_super_admin).await?;
    let previous_role_ids = repositories::users::role_ids_by_user(pool, user_id)
        .await
        .map_err(db_error)?;
    let mut transaction = pool.begin().await.map_err(db_error)?;
    if !super_admin_role_id.is_some_and(|id| req.role_ids.contains(&id)) {
        protect_last_super_admin(&mut transaction, user_id).await?;
    }
    repositories::users::assign_roles(&mut transaction, user_id, &req.role_ids)
        .await
        .map_err(db_error)?;
    repositories::audit_logs::record(
        &mut transaction,
        actor_user_id,
        "user.roles.update",
        "user",
        Some(user_id),
        json!({
            "previousRoleIds": previous_role_ids,
            "roleIds": req.role_ids,
        }),
    )
    .await
    .map_err(db_error)?;
    transaction.commit().await.map_err(db_error)?;
    Ok(())
}

async fn validate_role_grant_scope(
    pool: &PgPool,
    actor_user_id: Option<i64>,
    role_ids: &[i64],
    can_grant_super_admin: bool,
) -> Result<(), ApiError> {
    let super_admin_role_id = repositories::roles::id_by_code(pool, "super_admin")
        .await
        .map_err(db_error)?;
    if !can_grant_super_admin && super_admin_role_id.is_some_and(|id| role_ids.contains(&id)) {
        return Err(ApiError::forbidden("缺少授予超级管理员角色的权限"));
    }
    let Some(actor_user_id) = actor_user_id else {
        return Ok(());
    };
    let actor_permissions =
        repositories::permissions::permission_codes_by_user(pool, actor_user_id)
            .await
            .map_err(db_error)?
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
    let requested_permissions = repositories::permissions::codes_by_role_ids(pool, role_ids)
        .await
        .map_err(db_error)?;
    if requested_permissions
        .iter()
        .any(|permission| !actor_permissions.contains(permission))
    {
        return Err(ApiError::forbidden(
            "不能分配包含操作者自身未拥有权限的角色",
        ));
    }
    Ok(())
}

async fn protect_last_super_admin(
    connection: &mut sqlx::PgConnection,
    user_id: i64,
) -> Result<(), ApiError> {
    let (is_active_super_admin, active_count) =
        repositories::users::super_admin_guard_state(connection, user_id)
            .await
            .map_err(db_error)?;
    if !is_active_super_admin {
        return Ok(());
    }
    if active_count <= 1 {
        return Err(ApiError::forbidden(
            "不能停用、删除或移除最后一个处于启用状态的超级管理员",
        ));
    }
    Ok(())
}
