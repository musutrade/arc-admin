//! Opaque Cookie session authentication and typed permission extractors.

use crate::error::ApiError;
use crate::repositories;
use crate::AppState;
use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderName, Method};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::marker::PhantomData;
use subtle::ConstantTimeEq;
use time::Duration;

const SESSION_COOKIE: &str = "arc_session";
const SECURE_SESSION_COOKIE: &str = "__Host-arc_session";
const CSRF_COOKIE: &str = "arc_csrf";
const SECURE_CSRF_COOKIE: &str = "__Host-arc_csrf";
pub const CSRF_HEADER: HeaderName = HeaderName::from_static("x-csrf-token");

#[derive(Debug, Clone)]
pub struct AuthSessionConfig {
    pub session_ttl_secs: i64,
    pub session_idle_timeout_secs: i64,
    pub persistent_session_ttl_secs: i64,
    pub persistent_session_idle_timeout_secs: i64,
    pub max_sessions_per_user: i64,
    pub login_max_failures: i32,
    pub login_failure_window_secs: i64,
    pub login_lockout_secs: i64,
    pub secure_cookies: bool,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub session_id: i64,
    pub permission_codes: BTreeSet<String>,
}

pub fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex(&bytes)
}

pub fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

pub fn set_session_cookies(
    mut jar: CookieJar,
    session_token: String,
    csrf_token: String,
    persistent: bool,
    ttl_secs: i64,
    secure: bool,
) -> CookieJar {
    let mut session = Cookie::build((session_cookie_name(secure), session_token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Strict)
        .secure(secure);
    let mut csrf = Cookie::build((csrf_cookie_name(secure), csrf_token))
        .path("/")
        .http_only(false)
        .same_site(SameSite::Strict)
        .secure(secure);
    if persistent {
        let max_age = Duration::seconds(ttl_secs);
        session = session.max_age(max_age);
        csrf = csrf.max_age(max_age);
    }
    jar = jar.add(session.build());
    jar.add(csrf.build())
}

pub fn clear_session_cookies(mut jar: CookieJar, secure: bool) -> CookieJar {
    for (name, http_only) in [
        (session_cookie_name(secure), true),
        (csrf_cookie_name(secure), false),
    ] {
        let cookie = Cookie::build(name)
            .path("/")
            .http_only(http_only)
            .same_site(SameSite::Strict)
            .secure(secure)
            .build();
        jar = jar.remove(cookie);
    }
    jar
}

async fn authenticate(parts: &mut Parts, state: &AppState) -> Result<AuthUser, ApiError> {
    let jar = CookieJar::from_headers(&parts.headers);
    let session_token = jar
        .get(session_cookie_name(state.auth.secure_cookies))
        .map(Cookie::value)
        .filter(|value| value.len() == 64)
        .ok_or_else(ApiError::unauthorized)?;
    let context =
        repositories::auth_sessions::auth_context(&state.pool, &token_hash(session_token))
            .await
            .map_err(crate::error::db_error)?
            .ok_or_else(ApiError::unauthorized)?;

    validate_csrf(parts, &jar, &context.2, state.auth.secure_cookies)?;
    crate::telemetry::record_authenticated_user(context.1);
    Ok(AuthUser {
        session_id: context.0,
        user_id: context.1,
        permission_codes: context.3.into_iter().collect(),
    })
}

fn validate_csrf(
    parts: &Parts,
    jar: &CookieJar,
    expected_hash: &str,
    secure: bool,
) -> Result<(), ApiError> {
    if matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return Ok(());
    }
    let header_token = parts
        .headers
        .get(&CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() == 64)
        .ok_or_else(ApiError::csrf_invalid)?;
    let cookie_token = jar
        .get(csrf_cookie_name(secure))
        .map(Cookie::value)
        .filter(|value| value.len() == 64)
        .ok_or_else(ApiError::csrf_invalid)?;
    let values_match: bool = header_token
        .as_bytes()
        .ct_eq(cookie_token.as_bytes())
        .into();
    let hash_matches: bool = token_hash(header_token)
        .as_bytes()
        .ct_eq(expected_hash.as_bytes())
        .into();
    if !values_match || !hash_matches {
        return Err(ApiError::csrf_invalid());
    }
    Ok(())
}

fn session_cookie_name(secure: bool) -> &'static str {
    if secure {
        SECURE_SESSION_COOKIE
    } else {
        SESSION_COOKIE
    }
}

fn csrf_cookie_name(secure: bool) -> &'static str {
    if secure {
        SECURE_CSRF_COOKIE
    } else {
        CSRF_COOKIE
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    value
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        authenticate(parts, state).await
    }
}

pub trait PermissionRequirement {
    const CODE: &'static str;
}

pub struct RequirePermission<P> {
    pub user_id: i64,
    permission_codes: BTreeSet<String>,
    marker: PhantomData<P>,
}

impl<P> RequirePermission<P> {
    pub fn require(&self, code: &'static str) -> Result<(), ApiError> {
        if self.permission_codes.contains(code) {
            Ok(())
        } else {
            Err(ApiError::forbidden(format!("缺少权限：{code}")))
        }
    }

    pub fn has(&self, code: &'static str) -> bool {
        self.permission_codes.contains(code)
    }
}

impl<P> FromRequestParts<AppState> for RequirePermission<P>
where
    P: PermissionRequirement + Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = authenticate(parts, state).await?;
        if !auth.permission_codes.contains(P::CODE) {
            return Err(ApiError::forbidden(format!("缺少权限: {}", P::CODE)));
        }
        Ok(Self {
            user_id: auth.user_id,
            permission_codes: auth.permission_codes,
            marker: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::SET_COOKIE;
    use axum::response::IntoResponse;

    #[test]
    fn production_cookies_enforce_host_security_attributes() {
        let response = set_session_cookies(
            CookieJar::new(),
            random_token(),
            random_token(),
            true,
            3_600,
            true,
        )
        .into_response();
        let cookies = response
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .map(|value| value.to_str().expect("Set-Cookie header"))
            .collect::<Vec<_>>();
        let session = cookies
            .iter()
            .find(|cookie| cookie.starts_with("__Host-arc_session="))
            .expect("secure session cookie");
        let csrf = cookies
            .iter()
            .find(|cookie| cookie.starts_with("__Host-arc_csrf="))
            .expect("secure CSRF cookie");

        assert!(session.contains("HttpOnly"));
        assert!(session.contains("Secure"));
        assert!(session.contains("SameSite=Strict"));
        assert!(session.contains("Path=/"));
        assert!(session.contains("Max-Age=3600"));
        assert!(!csrf.contains("HttpOnly"));
        assert!(csrf.contains("Secure"));
        assert!(csrf.contains("SameSite=Strict"));
    }

    #[test]
    fn session_tokens_are_random_and_hashed_before_storage() {
        let first = random_token();
        let second = random_token();

        assert_eq!(first.len(), 64);
        assert_eq!(second.len(), 64);
        assert_ne!(first, second);
        assert_ne!(first, token_hash(&first));
        assert_eq!(token_hash(&first).len(), 64);
    }
}
