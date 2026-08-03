//! arc-admin 后端入口（Axum + SQLX + PostgreSQL）
//!
//! 分层约定（与 AGENTS.md / codex-audit-pipeline/.codex/audit.toml 保持一致）：
//!   handlers   ->  HTTP 层：参数校验、DTO 映射，禁止 SQL
//!   services   ->  业务逻辑，禁止 SQL
//!   repositories -> 唯一的 SQL 访问层（写操作只允许出现在这里）
//!   models     ->  纯数据结构
//!
//! 启动：DATABASE_URL=postgres://postgres:postgres@localhost:5432/arc_admin cargo run

use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::net::SocketAddr;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod db;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL 未设置，请参考 backend/.env.example");
    let pool = db::init_pool(&database_url).await?;

    // 启动时自动执行 backend/migrations 下尚未应用的迁移
    sqlx::migrate!("./migrations").run(&pool).await?;

    let app = Router::new()
        .route("/api/v1/healthz", get(healthz))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(AppState { pool });

    let addr: SocketAddr = "0.0.0.0:8080".parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

