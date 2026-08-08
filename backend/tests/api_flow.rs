use arc_admin_backend::models::{NullablePatch, UpdateUserRequest};
use arc_admin_backend::{build_router, db, services, AppState};
use axum::body::Body;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::Arc;
use tower::ServiceExt;

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = match body {
        Some(value) => {
            builder = builder.header(CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).expect("build request"))
        .await
        .expect("router response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("response JSON")
    };
    (status, value)
}

#[tokio::test]
async fn login_and_user_crud_flow() {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must point to an isolated test database");
    let pool = db::init_pool(&database_url).await.expect("test pool");
    db::run_migrations(&pool).await.expect("test migrations");

    let app = build_router(AppState {
        pool: pool.clone(),
        jwt_secret: Arc::new("integration-test-jwt-secret-at-least-32-chars".to_string()),
        token_ttl_secs: 3600,
    });

    let (status, health) = send(&app, Method::GET, "/api/v1/healthz", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["status"], "ok");

    let (status, readiness) = send(&app, Method::GET, "/api/v1/readyz", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(readiness["db"], true);

    let (status, _) = send(&app, Method::GET, "/api/v1/users", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({"username": "admin", "password": "admin123"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    services::auth::bootstrap_super_admin(
        &pool,
        "admin",
        "integration-admin-password",
        "Integration Administrator",
        Some("admin@example.test".to_string()),
    )
    .await
    .expect("bootstrap test administrator");

    let (status, login) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({
            "username": "admin",
            "password": "integration-admin-password"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let token = login["accessToken"].as_str().expect("access token");

    let (status, _) = send(
        &app,
        Method::PUT,
        "/api/v1/auth/me/password",
        None,
        Some(json!({
            "currentPassword": "integration-admin-password",
            "newPassword": "updated-integration-admin-password"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, error) = send(
        &app,
        Method::PUT,
        "/api/v1/auth/me/password",
        Some(token),
        Some(json!({
            "currentPassword": "integration-admin-password",
            "newPassword": "short"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error"]["message"], "新密码长度不能少于 8 位");

    let (status, error) = send(
        &app,
        Method::PUT,
        "/api/v1/auth/me/password",
        Some(token),
        Some(json!({
            "currentPassword": "integration-admin-password",
            "newPassword": "integration-admin-password"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error"]["message"], "新密码不能与当前密码相同");

    let (status, error) = send(
        &app,
        Method::PUT,
        "/api/v1/auth/me/password",
        Some(token),
        Some(json!({
            "currentPassword": "incorrect-password",
            "newPassword": "updated-integration-admin-password"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["error"]["message"], "当前密码不正确");

    let (status, _) = send(
        &app,
        Method::PUT,
        "/api/v1/auth/me/password",
        Some(token),
        Some(json!({
            "currentPassword": "integration-admin-password",
            "newPassword": "updated-integration-admin-password"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send(&app, Method::GET, "/api/v1/auth/me", Some(token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({
            "username": "admin",
            "password": "integration-admin-password"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, refreshed_login) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({
            "username": "admin",
            "password": "updated-integration-admin-password"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let token = refreshed_login["accessToken"]
        .as_str()
        .expect("refreshed access token");

    let (status, permission_groups) = send(
        &app,
        Method::GET,
        "/api/v1/permissions/groups",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let groups = permission_groups.as_array().expect("permission groups");
    assert_eq!(groups[0]["name"], "仪表盘模块");
    assert_eq!(groups[1]["name"], "身份与访问模块");
    assert_eq!(groups[2]["name"], "审计与合规模块");
    let group_codes = groups
        .iter()
        .filter_map(|group| group["code"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(group_codes, vec!["dashboard", "identity", "audit"]);
    let view_permissions = groups
        .iter()
        .flat_map(|group| group["permissions"].as_array().into_iter().flatten())
        .find(|permission| permission["code"] == "permission:directory:read")
        .expect("view permissions entry");
    assert_eq!(view_permissions["name"], "查看权限");
    let permission_codes = groups
        .iter()
        .flat_map(|group| {
            group["permissions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|permission| permission["code"].as_str())
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        permission_codes,
        BTreeSet::from([
            "dashboard:analytics:read",
            "audit:logs:read",
            "permission:directory:read",
            "role:directory:read",
            "role:permissions:write",
            "role:write",
            "user:admin:deactivate",
            "user:admin:reset_password",
            "user:directory:read",
            "user:roles:write",
            "user:super_admin:grant",
            "user:write",
        ])
    );
    let permission_id = |code: &str| {
        groups
            .iter()
            .flat_map(|group| group["permissions"].as_array().into_iter().flatten())
            .find(|permission| permission["code"] == code)
            .and_then(|permission| permission["id"].as_i64())
            .unwrap_or_else(|| panic!("missing permission {code}"))
    };
    let user_read_id = permission_id("user:directory:read");
    let user_write_id = permission_id("user:write");
    let user_roles_write_id = permission_id("user:roles:write");
    let role_read_id = permission_id("role:directory:read");
    let role_write_id = permission_id("role:write");
    let role_permission_write_id = permission_id("role:permissions:write");
    let permission_read_id = permission_id("permission:directory:read");

    let (status, roles) = send(&app, Method::GET, "/api/v1/roles", Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
    let viewer_role = roles
        .as_array()
        .expect("roles array")
        .iter()
        .find(|role| role["code"] == "viewer")
        .expect("viewer role");
    assert_eq!(viewer_role["name"], "查看者");
    assert_eq!(viewer_role["category"], "只读");
    let viewer_role_id = viewer_role["id"].as_i64().expect("viewer role id");
    let super_admin_role_id = roles
        .as_array()
        .expect("roles array")
        .iter()
        .find(|role| role["code"] == "super_admin")
        .and_then(|role| role["id"].as_i64())
        .expect("super admin role id");
    let support_role_id = roles
        .as_array()
        .expect("roles array")
        .iter()
        .find(|role| role["code"] == "support_tier2")
        .and_then(|role| role["id"].as_i64())
        .expect("support role id");

    let (status, allocator_role) = send(
        &app,
        Method::POST,
        "/api/v1/roles",
        Some(token),
        Some(json!({
            "code": "role_allocator",
            "name": "角色分配员",
            "permissionIds": [user_read_id, user_write_id, user_roles_write_id, role_read_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let allocator_role_id = allocator_role["id"].as_i64().expect("allocator role id");
    let (status, allocator) = send(
        &app,
        Method::POST,
        "/api/v1/users",
        Some(token),
        Some(json!({
            "username": "role_allocator",
            "password": "integration-pass",
            "displayName": "Role Allocator",
            "roleIds": [allocator_role_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let allocator_id = allocator["id"].as_i64().expect("allocator user id");
    let (status, allocator_login) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({"username": "role_allocator", "password": "integration-pass"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let allocator_token = allocator_login["accessToken"]
        .as_str()
        .expect("allocator token");
    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/users/{allocator_id}/roles"),
        Some(allocator_token),
        Some(json!({"roleIds": [super_admin_role_id]})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, support) = send(
        &app,
        Method::POST,
        "/api/v1/users",
        Some(token),
        Some(json!({
            "username": "support_actor",
            "password": "integration-pass",
            "displayName": "Support Actor",
            "roleIds": [support_role_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let support_id = support["id"].as_i64().expect("support user id");
    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/users/{support_id}/roles"),
        Some(token),
        Some(json!({"roleIds": [support_role_id]})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, support_login) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({"username": "support_actor", "password": "integration-pass"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let support_token = support_login["accessToken"]
        .as_str()
        .expect("support token");
    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/users/{support_id}/roles"),
        Some(support_token),
        Some(json!({"roleIds": [super_admin_role_id]})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, suspended_target) = send(
        &app,
        Method::POST,
        "/api/v1/users",
        Some(token),
        Some(json!({
            "username": "suspended_target",
            "password": "integration-pass",
            "displayName": "Suspended Target",
            "status": "suspended"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let suspended_target_id = suspended_target["id"]
        .as_i64()
        .expect("suspended target id");
    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/users/{suspended_target_id}"),
        Some(support_token),
        Some(json!({"status": "active"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, role_writer_role) = send(
        &app,
        Method::POST,
        "/api/v1/roles",
        Some(token),
        Some(json!({
            "code": "role_writer",
            "name": "角色编辑员",
            "permissionIds": [role_read_id, role_write_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let role_writer_role_id = role_writer_role["id"]
        .as_i64()
        .expect("role writer role id");
    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/users/{allocator_id}/roles"),
        Some(allocator_token),
        Some(json!({"roleIds": [role_writer_role_id]})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/users",
        Some(token),
        Some(json!({
            "username": "role_writer",
            "password": "integration-pass",
            "displayName": "Role Writer",
            "roleIds": [role_writer_role_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, role_writer_login) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({"username": "role_writer", "password": "integration-pass"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let role_writer_token = role_writer_login["accessToken"]
        .as_str()
        .expect("role writer token");
    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/roles",
        Some(role_writer_token),
        Some(json!({
            "code": "privileged_role",
            "name": "Privileged Role",
            "permissionIds": [user_write_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, permission_manager_role) = send(
        &app,
        Method::POST,
        "/api/v1/roles",
        Some(token),
        Some(json!({
            "code": "permission_manager",
            "name": "权限分配员",
            "permissionIds": [role_read_id, role_permission_write_id, permission_read_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let permission_manager_role_id = permission_manager_role["id"]
        .as_i64()
        .expect("permission manager role id");
    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/users",
        Some(token),
        Some(json!({
            "username": "permission_manager",
            "password": "integration-pass",
            "displayName": "Permission Manager",
            "roleIds": [permission_manager_role_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, permission_manager_login) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({"username": "permission_manager", "password": "integration-pass"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let permission_manager_token = permission_manager_login["accessToken"]
        .as_str()
        .expect("permission manager token");

    let admin_id = login["user"]["id"].as_i64().expect("administrator id");
    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/users/{admin_id}"),
        Some(token),
        Some(json!({"status": "inactive"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/roles/{super_admin_role_id}"),
        Some(token),
        Some(json!({"isActive": false})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/roles/{super_admin_role_id}/permissions"),
        Some(token),
        Some(json!({"permissionIds": []})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/users",
        Some(token),
        Some(json!({
            "username": "rolled_back_user",
            "password": "integration-pass",
            "displayName": "Rolled Back User",
            "roleIds": [i64::MAX]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, users) = send(
        &app,
        Method::GET,
        "/api/v1/users?keyword=rolled_back_user",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(users["total"], 0);

    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/roles",
        Some(token),
        Some(json!({
            "code": "rolled_back_role",
            "name": "Rolled Back Role",
            "permissionIds": [i64::MAX]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, roles_after_failure) =
        send(&app, Method::GET, "/api/v1/roles", Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(roles_after_failure
        .as_array()
        .expect("roles array")
        .iter()
        .all(|role| role["code"] != "rolled_back_role"));

    let (status, created) = send(
        &app,
        Method::POST,
        "/api/v1/users",
        Some(token),
        Some(json!({
            "username": "integration_user",
            "password": "integration-pass",
            "displayName": "Integration User",
            "email": "integration@example.com",
            "roleIds": [viewer_role_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let user_id = created["id"].as_i64().expect("created user id");
    assert_eq!(created["roles"], json!(["查看者"]));

    let (status, _) = send(
        &app,
        Method::DELETE,
        &format!("/api/v1/roles/{viewer_role_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, updated) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/users/{user_id}"),
        Some(token),
        Some(json!({"email": null})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(updated["email"].is_null());

    let (status, clearable_role) = send(
        &app,
        Method::POST,
        "/api/v1/roles",
        Some(token),
        Some(json!({
            "code": "clearable_role",
            "name": "Clearable Role",
            "icon": "badge",
            "description": "Temporary description"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let clearable_role_id = clearable_role["id"].as_i64().expect("clearable role id");
    let (status, error) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/roles/{clearable_role_id}/permissions"),
        Some(token),
        Some(json!({"permissionIds": [role_permission_write_id]})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        error["error"]["message"],
        "分配角色权限管理能力时，必须同时授予角色读取和权限目录读取权限"
    );
    let (status, error) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/roles/{clearable_role_id}/permissions"),
        Some(permission_manager_token),
        Some(json!({
            "permissionIds": [
                role_read_id,
                role_permission_write_id,
                permission_read_id,
                user_write_id
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["error"]["message"], "不能授予操作者自身未拥有的权限");
    let (status, clearable_role) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/roles/{clearable_role_id}"),
        Some(token),
        Some(json!({"icon": null, "description": null})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(clearable_role["icon"].is_null());
    assert!(clearable_role["description"].is_null());

    let (status, viewer_login) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({
            "username": "integration_user",
            "password": "integration-pass"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let viewer_token = viewer_login["accessToken"]
        .as_str()
        .expect("viewer access token");

    let (status, deactivated_role) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/roles/{viewer_role_id}"),
        Some(token),
        Some(json!({"isActive": false})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deactivated_role["isActive"], false);
    assert_eq!(deactivated_role["members"], 1);

    let (status, permissions_without_role) = send(
        &app,
        Method::GET,
        "/api/v1/auth/me/permissions",
        Some(viewer_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(permissions_without_role["codes"], json!([]));

    let (status, reactivated_role) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/roles/{viewer_role_id}"),
        Some(token),
        Some(json!({"isActive": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reactivated_role["isActive"], true);
    assert_eq!(reactivated_role["members"], 1);

    let (status, restored_permissions) = send(
        &app,
        Method::GET,
        "/api/v1/auth/me/permissions",
        Some(viewer_token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(restored_permissions["codes"]
        .as_array()
        .expect("permission codes")
        .iter()
        .any(|code| code == "role:directory:read"));

    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/roles",
        Some(viewer_token),
        Some(json!({"code": "forbidden_role", "name": "Forbidden Role"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, updated) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/users/{user_id}"),
        Some(token),
        Some(json!({"status": "inactive"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["status"], "inactive");

    let (status, _) = send(&app, Method::GET, "/api/v1/roles", Some(viewer_token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({
            "username": "integration_user",
            "password": "integration-pass"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, users) = send(
        &app,
        Method::GET,
        "/api/v1/users?keyword=integration_user",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(users["total"], 1);

    let (status, _) = send(
        &app,
        Method::DELETE,
        &format!("/api/v1/users/{user_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = send(
        &app,
        Method::GET,
        &format!("/api/v1/users/{user_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, roles_after_user_delete) =
        send(&app, Method::GET, "/api/v1/roles", Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
    let viewer_after_delete = roles_after_user_delete
        .as_array()
        .expect("roles array")
        .iter()
        .find(|role| role["code"] == "viewer")
        .expect("viewer role");
    assert_eq!(viewer_after_delete["members"], 0);

    let (status, second_admin) = send(
        &app,
        Method::POST,
        "/api/v1/users",
        Some(token),
        Some(json!({
            "username": "second_admin",
            "password": "integration-pass",
            "displayName": "Second Administrator",
            "roleIds": [super_admin_role_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let second_admin_id = second_admin["id"].as_i64().expect("second admin id");

    let (status, error) = send(
        &app,
        Method::PUT,
        &format!("/api/v1/users/{admin_id}"),
        Some(token),
        Some(json!({"status": "inactive"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["error"]["message"], "不能停用当前登录账号");

    let (status, audit_logs) = send(
        &app,
        Method::GET,
        "/api/v1/audit-logs?page=1&pageSize=100&action=user.roles.update",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(audit_logs["total"].as_i64().unwrap_or_default() > 0);
    assert!(audit_logs["items"]
        .as_array()
        .expect("audit log items")
        .iter()
        .all(|entry| entry["action"] == "user.roles.update"));

    let deactivate = || UpdateUserRequest {
        display_name: None,
        email: NullablePatch::Missing,
        status: Some("inactive".to_string()),
        password: None,
    };
    let first_request = deactivate();
    let second_request = deactivate();
    let (first, second) = tokio::join!(
        services::users::update(&pool, admin_id, None, &first_request, false),
        services::users::update(&pool, second_admin_id, None, &second_request, false),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);

    sqlx::query(
        "UPDATE roles SET name = '自定义查看角色', category = 'Read Only' WHERE code = 'viewer'",
    )
    .execute(&pool)
    .await
    .expect("prepare customized role");
    sqlx::raw_sql(include_str!(
        "../migrations/0007_localize_default_copy_zh_cn.sql"
    ))
    .execute(&pool)
    .await
    .expect("rerun localization migration");
    let localized_role = sqlx::query_as::<_, (String, String)>(
        "SELECT name, category FROM roles WHERE code = 'viewer'",
    )
    .fetch_one(&pool)
    .await
    .expect("localized viewer role");
    assert_eq!(localized_role.0, "自定义查看角色");
    assert_eq!(localized_role.1, "只读");
}
