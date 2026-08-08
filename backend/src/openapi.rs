//! OpenAPI contract generated from Rust DTOs and operation declarations.
#![allow(dead_code)]

use crate::error::ErrorEnvelope;
use crate::models::{
    AssignRolesRequest, AuditLogQuery, ChangePasswordRequest, CreateRoleRequest, CreateUserRequest,
    DashboardStats, DataScopeSchema, HealthResponse, LoginRequest, LoginResponse, PageAuditLog,
    PageQuery, PageUser, PermissionCodes, PermissionGroupResponse, PermissionResponse,
    PermissionTypeSchema, ReadinessResponse, RoleColorSchema, RolePermissions, RoleResponse,
    SortDirectionSchema, UpdateRolePermissionsRequest, UpdateRoleRequest, UpdateUserRequest,
    UserResponse, UserSortBySchema, UserStatusSchema,
};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::openapi::OpenApi as OpenApiDocument;
use utoipa::{Modify, OpenApi};

const COOKIE_SECURITY: &str = "cookieAuth";

#[utoipa::path(
    get,
    path = "/healthz",
    operation_id = "healthCheck",
    tag = "system",
    responses((status = 200, description = "进程存活", body = HealthResponse))
)]
fn health_check() {}

#[utoipa::path(
    get,
    path = "/readyz",
    operation_id = "readinessCheck",
    tag = "system",
    responses(
        (status = 200, description = "服务就绪", body = ReadinessResponse),
        (status = 503, description = "依赖不可用", body = ReadinessResponse)
    )
)]
fn readiness_check() {}

#[utoipa::path(
    post,
    path = "/auth/login",
    operation_id = "login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "登录成功", body = LoginResponse),
        (status = 401, description = "凭据错误", body = ErrorEnvelope),
        (status = 429, description = "登录尝试过于频繁", body = ErrorEnvelope)
    )
)]
fn login() {}

#[utoipa::path(
    post,
    path = "/auth/logout",
    operation_id = "logout",
    tag = "auth",
    security(("cookieAuth" = [])),
    params(("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")),
    responses(
        (status = 204, description = "退出成功"),
        (status = 401, description = "未认证", body = ErrorEnvelope),
        (status = 403, description = "CSRF 校验失败", body = ErrorEnvelope)
    )
)]
fn logout() {}

#[utoipa::path(
    get,
    path = "/auth/me",
    operation_id = "getCurrentUser",
    tag = "auth",
    security(("cookieAuth" = [])),
    responses(
        (status = 200, description = "当前用户", body = UserResponse),
        (status = 401, description = "未认证", body = ErrorEnvelope)
    )
)]
fn current_user() {}

#[utoipa::path(
    put,
    path = "/auth/me/password",
    operation_id = "changeCurrentUserPassword",
    tag = "auth",
    security(("cookieAuth" = [])),
    params(("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")),
    request_body = ChangePasswordRequest,
    responses(
        (status = 204, description = "密码已修改，现有会话已撤销"),
        (status = 401, description = "未认证", body = ErrorEnvelope),
        (status = 422, description = "密码不符合要求", body = ErrorEnvelope)
    )
)]
fn change_current_user_password() {}

#[utoipa::path(
    get,
    path = "/auth/me/permissions",
    operation_id = "getCurrentUserPermissions",
    tag = "auth",
    security(("cookieAuth" = [])),
    responses(
        (status = 200, description = "有效权限码", body = PermissionCodes),
        (status = 401, description = "未认证", body = ErrorEnvelope)
    )
)]
fn current_user_permissions() {}

#[utoipa::path(
    get,
    path = "/users",
    operation_id = "listUsers",
    tag = "users",
    security(("cookieAuth" = [])),
    params(PageQuery),
    responses(
        (status = 200, description = "用户分页结果", body = PageUser),
        (status = 401, description = "未认证", body = ErrorEnvelope),
        (status = 403, description = "无权读取用户", body = ErrorEnvelope),
        (status = 422, description = "查询参数无效", body = ErrorEnvelope)
    )
)]
fn list_users() {}

#[utoipa::path(
    post,
    path = "/users",
    operation_id = "createUser",
    tag = "users",
    security(("cookieAuth" = [])),
    params(("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")),
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "用户已创建", body = UserResponse),
        (status = 403, description = "无权创建用户", body = ErrorEnvelope),
        (status = 409, description = "用户名冲突", body = ErrorEnvelope),
        (status = 422, description = "请求无效", body = ErrorEnvelope)
    )
)]
fn create_user() {}

#[utoipa::path(
    get,
    path = "/users/{id}",
    operation_id = "getUser",
    tag = "users",
    security(("cookieAuth" = [])),
    params(("id" = i64, Path, description = "用户 ID")),
    responses(
        (status = 200, description = "用户详情", body = UserResponse),
        (status = 404, description = "用户不存在或超出数据范围", body = ErrorEnvelope)
    )
)]
fn get_user() {}

#[utoipa::path(
    put,
    path = "/users/{id}",
    operation_id = "updateUser",
    tag = "users",
    security(("cookieAuth" = [])),
    params(
        ("id" = i64, Path, description = "用户 ID"),
        ("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")
    ),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "用户已更新", body = UserResponse),
        (status = 403, description = "无权更新用户", body = ErrorEnvelope),
        (status = 404, description = "用户不存在或超出数据范围", body = ErrorEnvelope),
        (status = 422, description = "请求无效", body = ErrorEnvelope)
    )
)]
fn update_user() {}

#[utoipa::path(
    delete,
    path = "/users/{id}",
    operation_id = "deleteUser",
    tag = "users",
    security(("cookieAuth" = [])),
    params(
        ("id" = i64, Path, description = "用户 ID"),
        ("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")
    ),
    responses(
        (status = 204, description = "用户已删除"),
        (status = 403, description = "无权删除用户", body = ErrorEnvelope),
        (status = 404, description = "用户不存在或超出数据范围", body = ErrorEnvelope)
    )
)]
fn delete_user() {}

#[utoipa::path(
    put,
    path = "/users/{id}/roles",
    operation_id = "assignUserRoles",
    tag = "users",
    security(("cookieAuth" = [])),
    params(
        ("id" = i64, Path, description = "用户 ID"),
        ("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")
    ),
    request_body = AssignRolesRequest,
    responses(
        (status = 204, description = "角色已更新"),
        (status = 403, description = "无权分配角色", body = ErrorEnvelope),
        (status = 404, description = "用户不存在或超出数据范围", body = ErrorEnvelope),
        (status = 422, description = "角色无效", body = ErrorEnvelope)
    )
)]
fn assign_user_roles() {}

#[utoipa::path(
    get,
    path = "/roles",
    operation_id = "listRoles",
    tag = "roles",
    security(("cookieAuth" = [])),
    responses((status = 200, description = "角色列表", body = [RoleResponse]))
)]
fn list_roles() {}

#[utoipa::path(
    post,
    path = "/roles",
    operation_id = "createRole",
    tag = "roles",
    security(("cookieAuth" = [])),
    params(("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")),
    request_body = CreateRoleRequest,
    responses(
        (status = 201, description = "角色已创建", body = RoleResponse),
        (status = 403, description = "无权创建角色", body = ErrorEnvelope),
        (status = 409, description = "角色编码冲突", body = ErrorEnvelope),
        (status = 422, description = "请求无效", body = ErrorEnvelope)
    )
)]
fn create_role() {}

#[utoipa::path(
    get,
    path = "/roles/{id}",
    operation_id = "getRole",
    tag = "roles",
    security(("cookieAuth" = [])),
    params(("id" = i64, Path, description = "角色 ID")),
    responses(
        (status = 200, description = "角色详情", body = RoleResponse),
        (status = 404, description = "角色不存在", body = ErrorEnvelope)
    )
)]
fn get_role() {}

#[utoipa::path(
    put,
    path = "/roles/{id}",
    operation_id = "updateRole",
    tag = "roles",
    security(("cookieAuth" = [])),
    params(
        ("id" = i64, Path, description = "角色 ID"),
        ("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")
    ),
    request_body = UpdateRoleRequest,
    responses(
        (status = 200, description = "角色已更新", body = RoleResponse),
        (status = 403, description = "无权更新角色", body = ErrorEnvelope),
        (status = 404, description = "角色不存在", body = ErrorEnvelope),
        (status = 422, description = "请求无效", body = ErrorEnvelope)
    )
)]
fn update_role() {}

#[utoipa::path(
    delete,
    path = "/roles/{id}",
    operation_id = "deleteRole",
    tag = "roles",
    security(("cookieAuth" = [])),
    params(
        ("id" = i64, Path, description = "角色 ID"),
        ("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")
    ),
    responses(
        (status = 204, description = "角色已删除"),
        (status = 403, description = "无权删除角色", body = ErrorEnvelope),
        (status = 404, description = "角色不存在", body = ErrorEnvelope),
        (status = 409, description = "角色仍被使用", body = ErrorEnvelope)
    )
)]
fn delete_role() {}

#[utoipa::path(
    get,
    path = "/roles/{id}/permissions",
    operation_id = "getRolePermissions",
    tag = "roles",
    security(("cookieAuth" = [])),
    params(("id" = i64, Path, description = "角色 ID")),
    responses(
        (status = 200, description = "角色权限", body = RolePermissions),
        (status = 404, description = "角色不存在", body = ErrorEnvelope)
    )
)]
fn get_role_permissions() {}

#[utoipa::path(
    put,
    path = "/roles/{id}/permissions",
    operation_id = "updateRolePermissions",
    tag = "roles",
    security(("cookieAuth" = [])),
    params(
        ("id" = i64, Path, description = "角色 ID"),
        ("X-CSRF-Token" = String, Header, description = "当前会话的 CSRF Token")
    ),
    request_body = UpdateRolePermissionsRequest,
    responses(
        (status = 204, description = "角色权限已更新"),
        (status = 403, description = "无权分配权限", body = ErrorEnvelope),
        (status = 404, description = "角色不存在", body = ErrorEnvelope),
        (status = 422, description = "权限无效", body = ErrorEnvelope)
    )
)]
fn update_role_permissions() {}

#[utoipa::path(
    get,
    path = "/permissions/groups",
    operation_id = "listPermissionGroups",
    tag = "permissions",
    security(("cookieAuth" = [])),
    responses((status = 200, description = "权限组树", body = [PermissionGroupResponse]))
)]
fn list_permission_groups() {}

#[utoipa::path(
    get,
    path = "/dashboard/stats",
    operation_id = "getDashboardStats",
    tag = "dashboard",
    security(("cookieAuth" = [])),
    responses((status = 200, description = "仪表盘统计", body = DashboardStats))
)]
fn dashboard_stats() {}

#[utoipa::path(
    get,
    path = "/audit-logs",
    operation_id = "listAuditLogs",
    tag = "audit",
    security(("cookieAuth" = [])),
    params(AuditLogQuery),
    responses(
        (status = 200, description = "审计日志分页结果", body = PageAuditLog),
        (status = 403, description = "无权读取审计日志", body = ErrorEnvelope)
    )
)]
fn list_audit_logs() {}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Arc Admin API",
        version = "2.3.0",
        description = "Arc Admin 母模板的认证、RBAC、用户与审计 API"
    ),
    paths(
        health_check,
        readiness_check,
        login,
        logout,
        current_user,
        change_current_user_password,
        current_user_permissions,
        list_users,
        create_user,
        get_user,
        update_user,
        delete_user,
        assign_user_roles,
        list_roles,
        create_role,
        get_role,
        update_role,
        delete_role,
        get_role_permissions,
        update_role_permissions,
        list_permission_groups,
        dashboard_stats,
        list_audit_logs
    ),
    components(schemas(
        ErrorEnvelope,
        UserStatusSchema,
        DataScopeSchema,
        RoleColorSchema,
        PermissionTypeSchema,
        UserSortBySchema,
        SortDirectionSchema,
        HealthResponse,
        ReadinessResponse,
        UserResponse,
        RoleResponse,
        PermissionResponse,
        PermissionGroupResponse,
        LoginRequest,
        ChangePasswordRequest,
        LoginResponse,
        PageUser,
        PageAuditLog,
        CreateUserRequest,
        UpdateUserRequest,
        AssignRolesRequest,
        CreateRoleRequest,
        UpdateRoleRequest,
        RolePermissions,
        UpdateRolePermissionsRequest,
        PermissionCodes,
        DashboardStats
    )),
    servers((url = "/api/v1", description = "默认 API 根路径")),
    modifiers(&SecurityAddon),
    tags(
        (name = "system", description = "健康检查"),
        (name = "auth", description = "认证与当前会话"),
        (name = "users", description = "用户管理"),
        (name = "roles", description = "角色管理"),
        (name = "permissions", description = "权限目录"),
        (name = "dashboard", description = "仪表盘"),
        (name = "audit", description = "安全审计")
    )
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut OpenApiDocument) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                COOKIE_SECURITY,
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("__Host-arc_session"))),
            );
        }
    }
}

pub fn document() -> OpenApiDocument {
    ApiDoc::openapi()
}
