use arc_admin_backend::{build_router, db, AppState};
use axum::body::Body;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
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
        pool,
        jwt_secret: Arc::new("integration-test-jwt-secret-at-least-32-chars".to_string()),
        token_ttl_secs: 3600,
    });

    let (status, _) = send(&app, Method::GET, "/api/v1/users", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, login) = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({"username": "admin", "password": "admin123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let token = login["accessToken"].as_str().expect("access token");

    let (status, roles) = send(&app, Method::GET, "/api/v1/roles", Some(token), None).await;
    assert_eq!(status, StatusCode::OK);
    let viewer_role_id = roles
        .as_array()
        .expect("roles array")
        .iter()
        .find(|role| role["code"] == "viewer")
        .and_then(|role| role["id"].as_i64())
        .expect("viewer role id");

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
    assert_eq!(created["roles"], json!(["Viewer"]));

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
}
