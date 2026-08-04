//! arc-admin backend server entry point.

use arc_admin_backend::config::AppConfig;
use arc_admin_backend::{build_router, db, AppState};
use std::{net::SocketAddr, sync::Arc};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    if env_path.is_file() {
        dotenvy::from_path(&env_path)
            .map_err(|error| anyhow::anyhow!("failed to load backend/.env: {error}"))?;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let config = AppConfig::from_env()?;
    if config.uses_development_jwt {
        tracing::warn!("JWT_SECRET is not set; using the development-only default");
    }

    let pool = db::init_pool(&config.database_url).await?;
    let migrations = db::run_migrations(&pool).await?;
    tracing::info!(
        applied_migrations = migrations.applied,
        embedded_migrations = migrations.embedded,
        "database migrations ready"
    );

    let state = AppState {
        pool,
        jwt_secret: Arc::new(config.jwt_secret.clone()),
        token_ttl_secs: config.token_ttl_secs,
    };
    let app = build_router(state).layer(config.cors_layer());

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install termination handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
