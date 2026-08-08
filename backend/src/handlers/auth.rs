//! 认证 Handler：登录 / 当前用户 / 权限码

use crate::access::ActorContext;
use crate::auth;
use crate::error::ApiError;
use crate::models::{ChangePasswordRequest, LoginRequest, PermissionCodes, UserResponse};
use crate::services;
use crate::AppState;
use axum::extract::{ConnectInfo, State};
use axum::http::header::{CACHE_CONTROL, PRAGMA};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use std::net::SocketAddr;

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let client_ip = auth::resolve_client_ip(peer.ip(), &headers, &state.auth.trusted_proxy_cidrs);
    let outcome = services::auth::login(&state.pool, &state.auth, &req, client_ip).await?;
    let jar = auth::set_session_cookies(
        jar,
        outcome.session_token,
        outcome.csrf_token,
        outcome.persistent,
        outcome.ttl_secs,
        state.auth.secure_cookies,
    );
    let no_store = [
        (CACHE_CONTROL, HeaderValue::from_static("no-store")),
        (PRAGMA, HeaderValue::from_static("no-cache")),
    ];
    Ok((jar, no_store, Json(outcome.response)))
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    auth_user: ActorContext,
) -> Result<impl IntoResponse, ApiError> {
    services::auth::logout(&state.pool, auth_user.user_id, auth_user.session_id).await?;
    Ok((
        auth::clear_session_cookies(jar, state.auth.secure_cookies),
        StatusCode::NO_CONTENT,
    ))
}

pub async fn me(
    State(state): State<AppState>,
    auth: ActorContext,
) -> Result<Json<UserResponse>, ApiError> {
    services::auth::me(&state.pool, auth.user_id)
        .await
        .map(Json)
}

pub async fn change_password(
    State(state): State<AppState>,
    jar: CookieJar,
    auth: ActorContext,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, ApiError> {
    services::auth::change_password(&state.pool, auth.user_id, &req).await?;
    Ok((
        auth::clear_session_cookies(jar, state.auth.secure_cookies),
        StatusCode::NO_CONTENT,
    ))
}

pub async fn me_permissions(auth: ActorContext) -> Result<Json<PermissionCodes>, ApiError> {
    Ok(Json(PermissionCodes {
        codes: auth.permission_codes.into_iter().collect(),
    }))
}
