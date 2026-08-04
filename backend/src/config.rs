//! Runtime configuration and production safety checks.

use anyhow::{bail, Context};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderValue, Method};
use tower_http::cors::{AllowOrigin, CorsLayer};

const DEVELOPMENT_JWT_SECRET: &str = "dev-jwt-secret-change-me";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnvironment {
    Development,
    Test,
    Production,
}

pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
    pub jwt_secret: String,
    pub token_ttl_secs: i64,
    pub environment: AppEnvironment,
    allowed_origins: Vec<HeaderValue>,
    pub uses_development_jwt: bool,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_values(
            std::env::var("DATABASE_URL").ok(),
            std::env::var("PORT").ok(),
            std::env::var("JWT_SECRET").ok(),
            std::env::var("TOKEN_TTL_SECS").ok(),
            std::env::var("APP_ENV").ok(),
            std::env::var("CORS_ALLOWED_ORIGINS").ok(),
        )
    }

    fn from_values(
        database_url: Option<String>,
        port: Option<String>,
        jwt_secret: Option<String>,
        token_ttl_secs: Option<String>,
        app_env: Option<String>,
        cors_allowed_origins: Option<String>,
    ) -> anyhow::Result<Self> {
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
        let token_ttl_secs = token_ttl_secs
            .as_deref()
            .unwrap_or("86400")
            .parse::<i64>()
            .context("TOKEN_TTL_SECS must be a positive integer")?;
        if token_ttl_secs <= 0 {
            bail!("TOKEN_TTL_SECS must be a positive integer");
        }

        let uses_development_jwt = jwt_secret.is_none();
        let jwt_secret = jwt_secret.unwrap_or_else(|| DEVELOPMENT_JWT_SECRET.to_string());
        if environment == AppEnvironment::Production
            && (jwt_secret == DEVELOPMENT_JWT_SECRET || jwt_secret.len() < 32)
        {
            bail!("production requires JWT_SECRET with at least 32 characters");
        }

        let allowed_origins = parse_origins(cors_allowed_origins)?;
        if environment == AppEnvironment::Production && allowed_origins.is_empty() {
            bail!("production requires CORS_ALLOWED_ORIGINS with at least one explicit origin");
        }

        Ok(Self {
            database_url,
            port,
            jwt_secret,
            token_ttl_secs,
            environment,
            allowed_origins,
            uses_development_jwt,
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
            .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    }
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

    fn config(
        environment: &str,
        jwt_secret: Option<&str>,
        origins: Option<&str>,
    ) -> anyhow::Result<AppConfig> {
        AppConfig::from_values(
            Some("postgres://localhost/test".to_string()),
            None,
            jwt_secret.map(str::to_string),
            None,
            Some(environment.to_string()),
            origins.map(str::to_string),
        )
    }

    #[test]
    fn development_has_safe_local_defaults() {
        let config = config("development", None, None).expect("development config");

        assert_eq!(config.environment, AppEnvironment::Development);
        assert!(config.uses_development_jwt);
    }

    #[test]
    fn production_requires_a_strong_jwt_secret() {
        let error = config(
            "production",
            Some("short"),
            Some("https://admin.example.com"),
        )
        .err()
        .expect("weak secret must fail");

        assert!(error.to_string().contains("at least 32 characters"));
    }

    #[test]
    fn production_requires_explicit_cors_origins() {
        let error = config(
            "production",
            Some("this-secret-is-at-least-thirty-two-characters"),
            None,
        )
        .err()
        .expect("missing origins must fail");

        assert!(error.to_string().contains("CORS_ALLOWED_ORIGINS"));
    }
}
