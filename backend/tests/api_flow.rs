use arc_admin_backend::auth::{self, AuthSessionConfig};
use arc_admin_backend::models::{NullablePatch, UpdateUserRequest};
use arc_admin_backend::{build_router, db, services, AppState};
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{Method, Request, Response, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Debug)]
struct TestSession {
    cookie_header: String,
    csrf_token: String,
    session_token: String,
    session_set_cookie: String,
    csrf_set_cookie: String,
}

async fn request(
    app: &Router,
    method: Method,
    uri: &str,
    session: Option<&TestSession>,
    body: Option<Value>,
    include_csrf: bool,
    forwarded_for: Option<&str>,
) -> Response<Body> {
    let mut builder = Request::builder().method(method.clone()).uri(uri);
    if let Some(forwarded_for) = forwarded_for {
        builder = builder.header("x-forwarded-for", forwarded_for);
    }
    if let Some(session) = session {
        builder = builder.header(COOKIE, &session.cookie_header);
        if include_csrf && !matches!(method, Method::GET | Method::HEAD | Method::OPTIONS) {
            builder = builder.header("x-csrf-token", &session.csrf_token);
        }
    }
    let body = match body {
        Some(value) => {
            builder = builder.header(CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    app.clone()
        .oneshot(builder.body(body).expect("build request"))
        .await
        .expect("router response")
}

async fn response_json(response: Response<Body>) -> (StatusCode, Value) {
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

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    session: Option<&TestSession>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    response_json(request(app, method, uri, session, body, true, None).await).await
}

async fn send_from_ip(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    forwarded_for: &str,
) -> (StatusCode, Value) {
    response_json(request(app, method, uri, None, body, true, Some(forwarded_for)).await).await
}

async fn login(
    app: &Router,
    username: &str,
    password: &str,
    remember: bool,
) -> (StatusCode, Value, Option<TestSession>) {
    let response = request(
        app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({
            "username": username,
            "password": password,
            "remember": remember,
        })),
        false,
        None,
    )
    .await;
    let cookies = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let session_set_cookie = cookies
        .iter()
        .find(|cookie| cookie.starts_with("arc_session="))
        .cloned();
    let csrf_set_cookie = cookies
        .iter()
        .find(|cookie| cookie.starts_with("arc_csrf="))
        .cloned();
    let test_session =
        session_set_cookie
            .zip(csrf_set_cookie)
            .map(|(session_set_cookie, csrf_set_cookie)| {
                let session_cookie = session_set_cookie
                    .split(';')
                    .next()
                    .expect("session cookie pair");
                let csrf_cookie = csrf_set_cookie.split(';').next().expect("CSRF cookie pair");
                let session_token = session_cookie
                    .split_once('=')
                    .expect("session cookie value")
                    .1
                    .to_string();
                let csrf_token = csrf_cookie
                    .split_once('=')
                    .expect("CSRF cookie value")
                    .1
                    .to_string();
                TestSession {
                    cookie_header: format!("{session_cookie}; {csrf_cookie}"),
                    csrf_token,
                    session_token,
                    session_set_cookie,
                    csrf_set_cookie,
                }
            });
    let (status, body) = response_json(response).await;
    (status, body, test_session)
}

#[tokio::test]
async fn login_and_user_crud_flow() {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL must point to an isolated test database");
    let pool = db::init_pool(&database_url).await.expect("test pool");
    db::run_migrations(&pool).await.expect("test migrations");

    let app = build_router(AppState {
        pool: pool.clone(),
        auth: Arc::new(AuthSessionConfig {
            session_ttl_secs: 3_600,
            session_idle_timeout_secs: 1_800,
            persistent_session_ttl_secs: 86_400,
            persistent_session_idle_timeout_secs: 3_600,
            max_sessions_per_user: 10,
            login_max_failures: 5,
            login_ip_max_failures: 6,
            login_account_ip_max_failures: 5,
            login_failure_window_secs: 900,
            login_lockout_secs: 900,
            trusted_proxy_cidrs: vec!["10.0.0.0/8".parse().expect("trusted proxy CIDR")],
            secure_cookies: false,
        }),
    })
    .layer(MockConnectInfo(
        "10.2.0.5:4567"
            .parse::<SocketAddr>()
            .expect("mock peer address"),
    ));

    let (status, health) = send(&app, Method::GET, "/api/v1/healthz", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["status"], "ok");

    let (status, readiness) = send(&app, Method::GET, "/api/v1/readyz", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(readiness["db"], true);

    let (status, _) = send(&app, Method::GET, "/api/v1/users", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    for attempt in 1..=5 {
        let (status, _) = send_from_ip(
            &app,
            Method::POST,
            "/api/v1/auth/login",
            Some(json!({
                "username": "rate_limited_missing_user",
                "password": "incorrect-password"
            })),
            "198.51.100.20",
        )
        .await;
        let expected = if attempt < 5 {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::TOO_MANY_REQUESTS
        };
        assert_eq!(status, expected);
    }
    for username in ["another_missing_user", "third_missing_user"] {
        let (status, _) = send_from_ip(
            &app,
            Method::POST,
            "/api/v1/auth/login",
            Some(json!({
                "username": username,
                "password": "incorrect-password"
            })),
            "198.51.100.20",
        )
        .await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    }

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

    let (status, admin_login, token) =
        login(&app, "admin", "integration-admin-password", false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(admin_login.get("accessToken").is_none());
    let token = token.expect("session cookies");
    assert!(token.session_set_cookie.contains("HttpOnly"));
    assert!(token.session_set_cookie.contains("SameSite=Strict"));
    assert!(!token.csrf_set_cookie.contains("HttpOnly"));
    assert!(token.csrf_set_cookie.contains("SameSite=Strict"));
    let stored_session_hash = sqlx::query_scalar::<_, String>(
        "SELECT session_token_hash FROM auth_sessions ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("stored session hash");
    assert_eq!(
        stored_session_hash.trim(),
        auth::token_hash(&token.session_token)
    );
    assert_ne!(stored_session_hash.trim(), token.session_token);
    let token = &token;

    let (status, csrf_error) = response_json(
        request(
            &app,
            Method::PUT,
            "/api/v1/auth/me/password",
            Some(token),
            Some(json!({
                "currentPassword": "integration-admin-password",
                "newPassword": "updated-integration-admin-password"
            })),
            false,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(csrf_error["error"]["code"], "CSRF_INVALID");

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
    assert_eq!(error["error"]["message"], "密码长度需在 12-128 个字符之间");

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

    let (status, _refreshed_login, token) =
        login(&app, "admin", "updated-integration-admin-password", false).await;
    assert_eq!(status, StatusCode::OK);
    let token = token.expect("refreshed session cookies");
    let token = &token;

    let (status, _, logout_session) =
        login(&app, "admin", "updated-integration-admin-password", true).await;
    assert_eq!(status, StatusCode::OK);
    let logout_session = logout_session.expect("persistent session cookies");
    assert!(logout_session.session_set_cookie.contains("Max-Age="));
    let (status, _) = send(
        &app,
        Method::POST,
        "/api/v1/auth/logout",
        Some(&logout_session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = send(
        &app,
        Method::GET,
        "/api/v1/auth/me",
        Some(&logout_session),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

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
    let (status, _, allocator_token) =
        login(&app, "role_allocator", "integration-pass", false).await;
    assert_eq!(status, StatusCode::OK);
    let allocator_token = allocator_token.expect("allocator session cookies");
    let allocator_token = &allocator_token;
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
    let (status, _, support_token) = login(&app, "support_actor", "integration-pass", false).await;
    assert_eq!(status, StatusCode::OK);
    let support_token = support_token.expect("support session cookies");
    let support_token = &support_token;
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
    let (status, _, role_writer_token) =
        login(&app, "role_writer", "integration-pass", false).await;
    assert_eq!(status, StatusCode::OK);
    let role_writer_token = role_writer_token.expect("role writer session cookies");
    let role_writer_token = &role_writer_token;
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
    let (status, _, permission_manager_token) =
        login(&app, "permission_manager", "integration-pass", false).await;
    assert_eq!(status, StatusCode::OK);
    let permission_manager_token =
        permission_manager_token.expect("permission manager session cookies");
    let permission_manager_token = &permission_manager_token;

    let admin_id = admin_login["user"]["id"]
        .as_i64()
        .expect("administrator id");
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

    let (status, _, viewer_token) =
        login(&app, "integration_user", "integration-pass", false).await;
    assert_eq!(status, StatusCode::OK);
    let viewer_token = viewer_token.expect("viewer session cookies");
    let viewer_token = &viewer_token;

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
    assert!(audit_logs["items"]
        .as_array()
        .expect("audit log items")
        .iter()
        .all(|entry| entry["traceId"]
            .as_str()
            .is_some_and(|trace_id| !trace_id.is_empty())));
    let trace_id = audit_logs["items"][0]["traceId"]
        .as_str()
        .expect("audit trace id");
    let (status, traced_audit_logs) = send(
        &app,
        Method::GET,
        &format!("/api/v1/audit-logs?keyword={trace_id}"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(traced_audit_logs["items"]
        .as_array()
        .expect("traced audit log items")
        .iter()
        .all(|entry| entry["traceId"] == trace_id));

    let (status, revocation_logs) = send(
        &app,
        Method::GET,
        "/api/v1/audit-logs?page=1&pageSize=100&action=auth.session.revoked",
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let revocation_reasons = revocation_logs["items"]
        .as_array()
        .expect("session revocation audit items")
        .iter()
        .filter_map(|entry| entry["details"]["reason"].as_str())
        .collect::<BTreeSet<_>>();
    assert!(revocation_reasons.contains("password_change"));
    assert!(revocation_reasons.contains("logout"));
    assert!(revocation_reasons.contains("account_status_changed"));

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
