//! Runtime configuration and production safety checks.

use crate::auth::CSRF_HEADER;
use crate::telemetry::REQUEST_ID_HEADER;
use anyhow::{bail, Context};
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnvironment {
    Development,
    Test,
    Production,
}

impl AppEnvironment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Production => "production",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
}

impl LogFormat {
    pub fn bootstrap_from_env() -> Self {
        match std::env::var("LOG_FORMAT").as_deref() {
            Ok("json") => Self::Json,
            Ok("pretty") => Self::Pretty,
            _ if matches!(
                std::env::var("APP_ENV").as_deref(),
                Ok("production" | "prod")
            ) =>
            {
                Self::Json
            }
            _ => Self::Pretty,
        }
    }
}

pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
    pub session_ttl_secs: i64,
    pub session_idle_timeout_secs: i64,
    pub persistent_session_ttl_secs: i64,
    pub persistent_session_idle_timeout_secs: i64,
    pub max_sessions_per_user: i64,
    pub login_max_failures: i32,
    pub login_failure_window_secs: i64,
    pub login_lockout_secs: i64,
    pub environment: AppEnvironment,
    pub log_format: LogFormat,
    pub service_name: String,
    allowed_origins: Vec<HeaderValue>,
}

#[derive(Default)]
struct ConfigValues {
    database_url: Option<String>,
    port: Option<String>,
    session_ttl_secs: Option<String>,
    session_idle_timeout_secs: Option<String>,
    persistent_session_ttl_secs: Option<String>,
    persistent_session_idle_timeout_secs: Option<String>,
    max_sessions_per_user: Option<String>,
    login_max_failures: Option<String>,
    login_failure_window_secs: Option<String>,
    login_lockout_secs: Option<String>,
    app_env: Option<String>,
    cors_allowed_origins: Option<String>,
    log_format: Option<String>,
    service_name: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_values(ConfigValues {
            database_url: std::env::var("DATABASE_URL").ok(),
            port: std::env::var("PORT").ok(),
            session_ttl_secs: std::env::var("SESSION_TTL_SECS").ok(),
            session_idle_timeout_secs: std::env::var("SESSION_IDLE_TIMEOUT_SECS").ok(),
            persistent_session_ttl_secs: std::env::var("PERSISTENT_SESSION_TTL_SECS").ok(),
            persistent_session_idle_timeout_secs: std::env::var(
                "PERSISTENT_SESSION_IDLE_TIMEOUT_SECS",
            )
            .ok(),
            max_sessions_per_user: std::env::var("MAX_SESSIONS_PER_USER").ok(),
            login_max_failures: std::env::var("LOGIN_MAX_FAILURES").ok(),
            login_failure_window_secs: std::env::var("LOGIN_FAILURE_WINDOW_SECS").ok(),
            login_lockout_secs: std::env::var("LOGIN_LOCKOUT_SECS").ok(),
            app_env: std::env::var("APP_ENV").ok(),
            cors_allowed_origins: std::env::var("CORS_ALLOWED_ORIGINS").ok(),
            log_format: std::env::var("LOG_FORMAT").ok(),
            service_name: std::env::var("SERVICE_NAME").ok(),
        })
    }

    fn from_values(values: ConfigValues) -> anyhow::Result<Self> {
        let ConfigValues {
            database_url,
            port,
            session_ttl_secs,
            session_idle_timeout_secs,
            persistent_session_ttl_secs,
            persistent_session_idle_timeout_secs,
            max_sessions_per_user,
            login_max_failures,
            login_failure_window_secs,
            login_lockout_secs,
            app_env,
            cors_allowed_origins,
            log_format,
            service_name,
        } = values;
        let database_url = database_url.context(
            "DATABASE_URL is required; configure it in backend/.env or the process environment",
        )?;
        let environment = match app_env.as_deref().unwrap_or("development") {
            "development" | "dev" => AppEnvironment::Development,
            "test" => AppEnvironment::Test,
            "production" | "prod" => AppEnvironment::Production,
            value => bail!("APP_ENV must be development, test, or production; got {value:?}"),
        };
        let port = port
            .as_deref()
            .unwrap_or("8080")
            .parse::<u16>()
            .context("PORT must be an integer between 1 and 65535")?;
        if port == 0 {
            bail!("PORT must be an integer between 1 and 65535");
        }
        let session_ttl_secs = positive_i64("SESSION_TTL_SECS", session_ttl_secs, 28_800)?;
        let session_idle_timeout_secs = positive_i64(
            "SESSION_IDLE_TIMEOUT_SECS",
            session_idle_timeout_secs,
            1_800,
        )?;
        let persistent_session_ttl_secs = positive_i64(
            "PERSISTENT_SESSION_TTL_SECS",
            persistent_session_ttl_secs,
            2_592_000,
        )?;
        let persistent_session_idle_timeout_secs = positive_i64(
            "PERSISTENT_SESSION_IDLE_TIMEOUT_SECS",
            persistent_session_idle_timeout_secs,
            604_800,
        )?;
        let max_sessions_per_user =
            positive_i64("MAX_SESSIONS_PER_USER", max_sessions_per_user, 10)?;
        let login_max_failures = positive_i32("LOGIN_MAX_FAILURES", login_max_failures, 5)?;
        let login_failure_window_secs =
            positive_i64("LOGIN_FAILURE_WINDOW_SECS", login_failure_window_secs, 900)?;
        let login_lockout_secs = positive_i64("LOGIN_LOCKOUT_SECS", login_lockout_secs, 900)?;
        if session_idle_timeout_secs > session_ttl_secs {
            bail!("SESSION_IDLE_TIMEOUT_SECS cannot exceed SESSION_TTL_SECS");
        }
        if persistent_session_idle_timeout_secs > persistent_session_ttl_secs {
            bail!("PERSISTENT_SESSION_IDLE_TIMEOUT_SECS cannot exceed PERSISTENT_SESSION_TTL_SECS");
        }

        let allowed_origins = parse_origins(cors_allowed_origins)?;
        if environment == AppEnvironment::Production && allowed_origins.is_empty() {
            bail!("production requires CORS_ALLOWED_ORIGINS with at least one explicit origin");
        }
        let log_format = match log_format.as_deref() {
            Some("pretty") => LogFormat::Pretty,
            Some("json") => LogFormat::Json,
            Some(value) => bail!("LOG_FORMAT must be pretty or json; got {value:?}"),
            None if environment == AppEnvironment::Production => LogFormat::Json,
            None => LogFormat::Pretty,
        };
        let service_name = service_name.unwrap_or_else(|| "arc-admin-backend".to_string());
        if service_name.is_empty()
            || service_name.len() > 64
            || !service_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            bail!("SERVICE_NAME must be 1-64 ASCII letters, digits, '.', '_' or '-'");
        }

        Ok(Self {
            database_url,
            port,
            session_ttl_secs,
            session_idle_timeout_secs,
            persistent_session_ttl_secs,
            persistent_session_idle_timeout_secs,
            max_sessions_per_user,
            login_max_failures,
            login_failure_window_secs,
            login_lockout_secs,
            environment,
            log_format,
            service_name,
            allowed_origins,
        })
    }

    pub fn cors_layer(&self) -> CorsLayer {
        if self.allowed_origins.is_empty() {
            return CorsLayer::permissive();
        }

        CorsLayer::new()
            .allow_origin(AllowOrigin::list(self.allowed_origins.clone()))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::OPTIONS,
            ])
            .allow_headers([CONTENT_TYPE, CSRF_HEADER, REQUEST_ID_HEADER])
            .expose_headers([REQUEST_ID_HEADER])
            .allow_credentials(true)
    }
}

fn positive_i64(name: &str, value: Option<String>, default: i64) -> anyhow::Result<i64> {
    let value = value
        .as_deref()
        .map(str::parse::<i64>)
        .transpose()
        .with_context(|| format!("{name} must be a positive integer"))?
        .unwrap_or(default);
    if value <= 0 {
        bail!("{name} must be a positive integer");
    }
    Ok(value)
}

fn positive_i32(name: &str, value: Option<String>, default: i32) -> anyhow::Result<i32> {
    let value = value
        .as_deref()
        .map(str::parse::<i32>)
        .transpose()
        .with_context(|| format!("{name} must be a positive integer"))?
        .unwrap_or(default);
    if value <= 0 {
        bail!("{name} must be a positive integer");
    }
    Ok(value)
}

fn parse_origins(value: Option<String>) -> anyhow::Result<Vec<HeaderValue>> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            if origin == "*" {
                bail!("CORS_ALLOWED_ORIGINS must contain explicit origins, not '*'");
            }
            origin
                .parse::<HeaderValue>()
                .with_context(|| format!("invalid CORS origin {origin:?}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(environment: &str, origins: Option<&str>) -> anyhow::Result<AppConfig> {
        AppConfig::from_values(ConfigValues {
            database_url: Some("postgres://localhost/test".to_string()),
            app_env: Some(environment.to_string()),
            cors_allowed_origins: origins.map(str::to_string),
            ..ConfigValues::default()
        })
    }

    #[test]
    fn development_has_safe_local_defaults() {
        let config = config("development", None).expect("development config");

        assert_eq!(config.environment, AppEnvironment::Development);
        assert_eq!(config.log_format, LogFormat::Pretty);
        assert_eq!(config.session_ttl_secs, 28_800);
        assert_eq!(config.login_max_failures, 5);
    }

    #[test]
    fn production_requires_explicit_cors_origins() {
        let error = config("production", None)
            .err()
            .expect("missing origins must fail");

        assert!(error.to_string().contains("CORS_ALLOWED_ORIGINS"));
    }

    #[test]
    fn production_uses_json_logs_by_default() {
        let config =
            config("production", Some("https://admin.example.com")).expect("production config");

        assert_eq!(config.log_format, LogFormat::Json);
    }

    #[test]
    fn idle_timeout_cannot_exceed_absolute_session_ttl() {
        let error = AppConfig::from_values(ConfigValues {
            database_url: Some("postgres://localhost/test".to_string()),
            session_ttl_secs: Some("60".to_string()),
            session_idle_timeout_secs: Some("61".to_string()),
            ..ConfigValues::default()
        })
        .err()
        .expect("invalid session timeouts must fail");

        assert!(error.to_string().contains("SESSION_IDLE_TIMEOUT_SECS"));
    }
}
