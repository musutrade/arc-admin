//! 仪表盘 Handler：统计指标

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::models::DashboardStats;
use crate::services;
use crate::AppState;
use axum::extract::State;
use axum::Json;

pub async fn stats(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<DashboardStats>, ApiError> {
    services::permissions::stats(&state.pool).await.map(Json)
}
