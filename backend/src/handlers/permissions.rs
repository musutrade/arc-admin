//! 权限 Handler：权限组树

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::models::PermissionGroupResponse;
use crate::services;
use crate::AppState;
use axum::extract::State;
use axum::Json;

pub async fn groups(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<Vec<PermissionGroupResponse>>, ApiError> {
    services::permissions::groups(&state.pool).await.map(Json)
}

