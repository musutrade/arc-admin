//! JWT 认证：Claims 定义 + AuthUser 提取器（从 Authorization: Bearer <token> 解码）

use crate::error::ApiError;
use crate::repositories;
use crate::AppState;
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::marker::PhantomData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: i64,
    pub permission_codes: BTreeSet<String>,
}

async fn authenticate(parts: &mut Parts, state: &AppState) -> Result<AuthUser, ApiError> {
    let header = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(ApiError::unauthorized)?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(ApiError::unauthorized)?;
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| ApiError::unauthorized())?;
    let user_id = data
        .claims
        .sub
        .parse::<i64>()
        .map_err(|_| ApiError::unauthorized())?;
    let (active, permission_codes) = repositories::permissions::auth_context(&state.pool, user_id)
        .await
        .map_err(crate::error::db_error)?;
    if !active {
        return Err(ApiError::unauthorized());
    }
    Ok(AuthUser {
        user_id,
        permission_codes: permission_codes.into_iter().collect(),
    })
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
            Err(ApiError::forbidden(format!("缺少权限: {code}")))
        }
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

macro_rules! permission {
    ($name:ident, $code:literal) => {
        pub struct $name;

        impl PermissionRequirement for $name {
            const CODE: &'static str = $code;
        }
    };
}

permission!(UserRead, "user:directory:read");
permission!(UserWrite, "user:write");
permission!(UserDeactivate, "user:admin:deactivate");
permission!(RoleRead, "role:directory:read");
permission!(RoleWrite, "role:write");
permission!(RolePermissionWrite, "role:permissions:write");
permission!(PermissionRead, "permission:directory:read");
permission!(DashboardRead, "dashboard:analytics:read");
