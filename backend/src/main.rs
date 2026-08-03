//! arc-admin 后端入口（Axum + SQLX + PostgreSQL）
//!
//! 分层约定（与 AGENTS.md / codex-audit-pipeline/.codex/audit.toml 保持一致）：
//!   handlers   ->  HTTP 层：参数校验、DTO 映射，禁止 SQL
//!   services   ->  业务逻辑，禁止 SQL
//!   repositories -> 唯一的 SQL 访问层（写操作只允许出现在这里）
//!   models     ->  纯数据结构
//!
//! 启动：DATABASE_URL=postgres://postgres:postgres@localhost:5432/arc_admin cargo run

use axum::extract::State;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{net::SocketAddr, sync::Arc};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod auth;
mod db;
mod error;
mod handlers;
mod models;
mod repositories;
mod services;

pub use error::ApiError;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: Arc<String>,
    pub token_ttl_secs: i64,
}

async fn healthz(State(state): State<AppState>) -> Json<Value> {
    let db_ok = db::ping(&state.pool).await;
    Json(json!({
        "status": if db_ok { "ok" } else { "degraded" },
        "db": db_ok,
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 优先加载 backend/.env（与 .env.example 同目录）；已导出的环境变量优先，不会被覆盖
    let env_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    if env_path.is_file() {
        dotenvy::from_path(&env_path)
            .map_err(|e| anyhow::anyhow!("加载 backend/.env 失败（{e}），请检查文件格式"))?;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL").map_err(|_| {
        anyhow::anyhow!(
            "DATABASE_URL 未设置：请在 backend/.env 中配置，例如 \
             DATABASE_URL=postgres://postgres:postgres@localhost:5432/arc_admin"
        )
    })?;
    let pool = db::init_pool(&database_url).await?;

    // 启动时自动执行 backend/migrations 下尚未应用的迁移
    sqlx::migrate!("./migrations").run(&pool).await?;

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        eprintln!("⚠️ JWT_SECRET 未设置，使用开发默认值（生产环境必须配置）");
        "dev-jwt-secret-change-me".to_string()
    });
    let token_ttl_secs: i64 = std::env::var("TOKEN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24 * 3600);

    let state = AppState {
        pool,
        jwt_secret: Arc::new(jwt_secret),
        token_ttl_secs,
    };

    let app = Router::new()
        .route("/api/v1/healthz", get(healthz))
        .route("/api/v1/auth/login", post(handlers::auth::login))
        .route("/api/v1/auth/me", get(handlers::auth::me))
        .route("/api/v1/auth/me/permissions", get(handlers::auth::me_permissions))
        .route(
            "/api/v1/users",
            get(handlers::users::list).post(handlers::users::create),
        )
        .route(
            "/api/v1/users/{id}",
            get(handlers::users::get)
                .put(handlers::users::update)
                .delete(handlers::users::delete),
        )
        .route("/api/v1/users/{id}/roles", put(handlers::users::assign_roles))
        .route(
            "/api/v1/roles",
            get(handlers::roles::list).post(handlers::roles::create),
        )
        .route(
            "/api/v1/roles/{id}",
            get(handlers::roles::get)
                .put(handlers::roles::update)
                .delete(handlers::roles::delete),
        )
        .route(
            "/api/v1/roles/{id}/permissions",
            get(handlers::roles::get_permissions).put(handlers::roles::put_permissions),
        )
        .route("/api/v1/permissions/groups", get(handlers::permissions::groups))
        .route("/api/v1/dashboard/stats", get(handlers::dashboard::stats))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;
    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
