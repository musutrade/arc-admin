//! arc-admin backend library: application wiring shared by the server and integration tests.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod handlers;
pub mod models;
pub mod permissions;
pub mod repositories;
pub mod services;
pub mod telemetry;

pub use error::ApiError;

pub const API_PREFIX: &str = "/api/v1";
const HEALTHZ_PATH: &str = "/api/v1/healthz";
const READYZ_PATH: &str = "/api/v1/readyz";
const LOGIN_PATH: &str = "/api/v1/auth/login";
const CURRENT_USER_PATH: &str = "/api/v1/auth/me";
const CURRENT_USER_PASSWORD_PATH: &str = "/api/v1/auth/me/password";
const CURRENT_USER_PERMISSIONS_PATH: &str = "/api/v1/auth/me/permissions";
const USERS_PATH: &str = "/api/v1/users";
const USER_PATH: &str = "/api/v1/users/{id}";
const USER_ROLES_PATH: &str = "/api/v1/users/{id}/roles";
const ROLES_PATH: &str = "/api/v1/roles";
const ROLE_PATH: &str = "/api/v1/roles/{id}";
const ROLE_PERMISSIONS_PATH: &str = "/api/v1/roles/{id}/permissions";
const PERMISSION_GROUPS_PATH: &str = "/api/v1/permissions/groups";
const DASHBOARD_STATS_PATH: &str = "/api/v1/dashboard/stats";
const AUDIT_LOGS_PATH: &str = "/api/v1/audit-logs";

/// Public HTTP operations documented in `docs/openapi.yaml`.
pub const API_ROUTE_CONTRACT: &[(&str, &[&str])] = &[
    (HEALTHZ_PATH, &["get"]),
    (READYZ_PATH, &["get"]),
    (LOGIN_PATH, &["post"]),
    (CURRENT_USER_PATH, &["get"]),
    (CURRENT_USER_PASSWORD_PATH, &["put"]),
    (CURRENT_USER_PERMISSIONS_PATH, &["get"]),
    (USERS_PATH, &["get", "post"]),
    (USER_PATH, &["get", "put", "delete"]),
    (USER_ROLES_PATH, &["put"]),
    (ROLES_PATH, &["get", "post"]),
    (ROLE_PATH, &["get", "put", "delete"]),
    (ROLE_PERMISSIONS_PATH, &["get", "put"]),
    (PERMISSION_GROUPS_PATH, &["get"]),
    (DASHBOARD_STATS_PATH, &["get"]),
    (AUDIT_LOGS_PATH, &["get"]),
];

/// Required response fields that must remain aligned with `docs/openapi.yaml`.
pub const API_SCHEMA_REQUIRED_FIELDS: &[(&str, &[&str])] = &[
    (
        "User",
        &[
            "id",
            "username",
            "displayName",
            "email",
            "status",
            "roles",
            "lastLoginAt",
            "createdAt",
        ],
    ),
    ("PageUser", &["items", "total", "page", "pageSize"]),
    (
        "LoginResponse",
        &["accessToken", "tokenType", "expiresIn", "user"],
    ),
    (
        "Role",
        &[
            "id",
            "code",
            "name",
            "category",
            "icon",
            "color",
            "description",
            "isActive",
            "members",
            "permissionGroupIds",
        ],
    ),
    ("RolePermissions", &["permissionIds"]),
    ("Permission", &["id", "code", "name", "type", "description"]),
    (
        "PermissionGroup",
        &["id", "code", "name", "icon", "permissions"],
    ),
    (
        "DashboardStats",
        &[
            "totalUsers",
            "activeUsers",
            "totalRoles",
            "totalPermissions",
            "suspendedUsers",
        ],
    ),
    (
        "AuditLog",
        &[
            "id",
            "actorUserId",
            "actorUsername",
            "action",
            "targetType",
            "targetId",
            "details",
            "traceId",
            "createdAt",
        ],
    ),
    ("PageAuditLog", &["items", "total", "page", "pageSize"]),
];

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: Arc<String>,
    pub token_ttl_secs: i64,
}

async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let db_ok = db::ping(&state.pool).await;
    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(json!({
            "status": if db_ok { "ok" } else { "degraded" },
            "db": db_ok,
        })),
    )
}

fn base_router(state: AppState) -> Router {
    Router::new()
        .route(HEALTHZ_PATH, get(healthz))
        .route(READYZ_PATH, get(readyz))
        .route(LOGIN_PATH, post(handlers::auth::login))
        .route(CURRENT_USER_PATH, get(handlers::auth::me))
        .route(
            CURRENT_USER_PASSWORD_PATH,
            put(handlers::auth::change_password),
        )
        .route(
            CURRENT_USER_PERMISSIONS_PATH,
            get(handlers::auth::me_permissions),
        )
        .route(
            USERS_PATH,
            get(handlers::users::list).post(handlers::users::create),
        )
        .route(
            USER_PATH,
            get(handlers::users::get)
                .put(handlers::users::update)
                .delete(handlers::users::delete),
        )
        .route(USER_ROLES_PATH, put(handlers::users::assign_roles))
        .route(
            ROLES_PATH,
            get(handlers::roles::list).post(handlers::roles::create),
        )
        .route(
            ROLE_PATH,
            get(handlers::roles::get)
                .put(handlers::roles::update)
                .delete(handlers::roles::delete),
        )
        .route(
            ROLE_PERMISSIONS_PATH,
            get(handlers::roles::get_permissions).put(handlers::roles::put_permissions),
        )
        .route(PERMISSION_GROUPS_PATH, get(handlers::permissions::groups))
        .route(DASHBOARD_STATS_PATH, get(handlers::dashboard::stats))
        .route(AUDIT_LOGS_PATH, get(handlers::audit_logs::list))
        .with_state(state)
}

pub fn build_router(state: AppState) -> Router {
    telemetry::default_http_observability(base_router(state))
}

pub fn build_router_with_metadata(
    state: AppState,
    metadata: telemetry::TelemetryMetadata,
) -> Router {
    telemetry::with_http_observability(base_router(state), metadata)
}

pub fn build_router_with_metadata_and_cors(
    state: AppState,
    metadata: telemetry::TelemetryMetadata,
    cors: CorsLayer,
) -> Router {
    telemetry::with_http_observability(base_router(state).layer(cors), metadata)
}
