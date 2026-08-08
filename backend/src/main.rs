//! arc-admin backend server entry point.

use arc_admin_backend::config::{AppConfig, LogFormat};
use arc_admin_backend::telemetry::{self, TelemetryMetadata};
use arc_admin_backend::{build_router_with_metadata_and_cors, db, AppState};
use std::process::ExitCode;
use std::{net::SocketAddr, sync::Arc};
use tracing::Instrument;

#[tokio::main]
async fn main() -> ExitCode {
    let env_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env");
    let environment_result = if env_path.is_file() {
        dotenvy::from_path(&env_path)
            .map(|_| ())
            .map_err(|error| anyhow::anyhow!("failed to load backend/.env: {error}"))
    } else {
        Ok(())
    };
    if let Err(error) = telemetry::init(LogFormat::bootstrap_from_env()) {
        eprintln!("failed to initialize logging: {error}");
        return ExitCode::FAILURE;
    }
    telemetry::install_panic_hook();
    if let Err(error) = environment_result {
        tracing::error!(
            event = "application.configuration_failure",
            error = %format!("{error:#}"),
            "application configuration failed"
        );
        return ExitCode::FAILURE;
    }

    let config = match AppConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(
                event = "application.configuration_failure",
                error = %format!("{error:#}"),
                "application configuration failed"
            );
            return ExitCode::FAILURE;
        }
    };
    let metadata = TelemetryMetadata::from_config(&config);
    let application_span = telemetry::application_span(&metadata);

    let result = async move {
        let result = run(config, metadata).await;
        if let Err(error) = &result {
            tracing::error!(
                event = "application.failure",
                error = %format!("{error:#}"),
                "application stopped with error"
            );
        }
        result
    }
    .instrument(application_span)
    .await;
    if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

async fn run(config: AppConfig, metadata: TelemetryMetadata) -> anyhow::Result<()> {
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
    let app = build_router_with_metadata_and_cors(state, metadata, config.cors_layer());

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
