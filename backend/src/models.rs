//! 数据模型：数据库行（FromRow）+ API DTO（serde）
//! 约定：本文件只放纯数据结构与派生宏；转换函数用自由函数，不放 impl 业务逻辑

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;

// ===== 数据库行 =====

#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub display_name: String,
    pub email: Option<String>,
    pub status: String,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserWithRolesRow {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub status: String,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct RoleRow {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub category: String,
    pub icon: Option<String>,
    pub color: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub members: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct RoleWithPermissionsRow {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub category: String,
    pub icon: Option<String>,
    pub color: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub members: i64,
    pub permission_group_ids: Vec<i64>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PermissionGroupRow {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PermissionRow {
    pub id: i64,
    pub group_id: i64,
    pub code: String,
    pub name: String,
    #[sqlx(rename = "type")]
    pub r#type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DashboardStatsRow {
    pub total_users: i64,
    pub active_users: i64,
    pub total_roles: i64,
    pub total_permissions: i64,
    pub suspended_users: i64,
}

// ===== API DTO =====

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub status: String,
    pub roles: Vec<String>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub fn user_response(row: UserRow, roles: Vec<String>) -> UserResponse {
    UserResponse {
        id: row.id,
        username: row.username,
        display_name: row.display_name,
        email: row.email,
        status: row.status,
        roles,
        last_login_at: row.last_login_at,
        created_at: row.created_at,
    }
}

pub fn user_with_roles_response(row: UserWithRolesRow) -> UserResponse {
    UserResponse {
        id: row.id,
        username: row.username,
        display_name: row.display_name,
        email: row.email,
        status: row.status,
        roles: row.roles,
        last_login_at: row.last_login_at,
        created_at: row.created_at,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleResponse {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub category: String,
    pub icon: Option<String>,
    pub color: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub members: i64,
    pub permission_group_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionResponse {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub r#type: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionGroupResponse {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub icon: Option<String>,
    pub permissions: Vec<PermissionResponse>,
}

// ===== 请求 / 响应 =====

#[derive(Debug, Default)]
pub enum NullablePatch<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<'de, T> Deserialize<'de> for NullablePatch<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

pub fn nullable_patch<T: Clone>(patch: &NullablePatch<T>) -> (bool, Option<T>) {
    match patch {
        NullablePatch::Missing => (false, None),
        NullablePatch::Null => (true, None),
        NullablePatch::Value(value) => (true, Some(value.clone())),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub keyword: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageUser {
    pub items: Vec<UserResponse>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub display_name: String,
    pub email: Option<String>,
    pub status: Option<String>,
    pub role_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    #[serde(default)]
    pub email: NullablePatch<String>,
    pub status: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignRolesRequest {
    pub role_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRoleRequest {
    pub code: String,
    pub name: String,
    pub category: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub permission_ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleRequest {
    pub name: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub icon: NullablePatch<String>,
    pub color: Option<String>,
    #[serde(default)]
    pub description: NullablePatch<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RolePermissions {
    pub permission_ids: Vec<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRolePermissionsRequest {
    pub permission_ids: Vec<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionCodes {
    pub codes: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub total_users: i64,
    pub active_users: i64,
    pub total_roles: i64,
    pub total_permissions: i64,
    pub suspended_users: i64,
}

impl From<DashboardStatsRow> for DashboardStats {
    fn from(row: DashboardStatsRow) -> Self {
        Self {
            total_users: row.total_users,
            active_users: row.active_users,
            total_roles: row.total_roles,
            total_permissions: row.total_permissions,
            suspended_users: row.suspended_users,
        }
    }
}
