//! 统一 API 错误：thiserror 定义 + axum IntoResponse 映射为安全错误契约。

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("未认证或登录已过期")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ApiError {
    pub fn unauthorized() -> Self {
        Self::Unauthorized
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        Self::Internal(anyhow::anyhow!("{error}"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let code = match &self {
            Self::Unauthorized => "UNAUTHORIZED",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::Internal(_) => "INTERNAL_ERROR",
        };
        let trace_id =
            crate::telemetry::current_trace_id().unwrap_or_else(|| "unavailable".to_string());
        let message = match &self {
            Self::Internal(error) => {
                tracing::error!(
                    event = "api.error",
                    trace_id = %trace_id,
                    error_code = code,
                    error = %format!("{error:#}"),
                    "internal API error"
                );
                "服务器内部错误".to_string()
            }
            other => other.to_string(),
        };
        error_response(status, code, message, trace_id)
    }
}

pub(crate) fn internal_error_response() -> Response {
    let trace_id =
        crate::telemetry::current_trace_id().unwrap_or_else(|| "unavailable".to_string());
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL_ERROR",
        "服务器内部错误".to_string(),
        trace_id,
    )
}

fn error_response(
    status: StatusCode,
    code: &'static str,
    message: String,
    trace_id: String,
) -> Response {
    let body = ErrorEnvelope {
        error: ErrorBody {
            code,
            message,
            trace_id,
        },
    };
    (status, Json(body)).into_response()
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    code: &'static str,
    message: String,
    trace_id: String,
}

/// sqlx 错误 → ApiError（唯一约束 → 409，外键 → 422，其余 → 500）
pub fn db_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(db) = &error {
        if db.is_unique_violation() {
            return ApiError::conflict("唯一约束冲突：用户名或角色编码已存在");
        }
        if db.is_foreign_key_violation() {
            return ApiError::validation("关联数据不存在或仍被引用");
        }
    }
    ApiError::internal(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn internal_error_response_does_not_expose_root_cause() {
        let response = ApiError::internal("database password leaked").into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect error response")
            .to_bytes();
        let body = String::from_utf8(body.to_vec()).expect("UTF-8 error response");
        assert!(body.contains("服务器内部错误"));
        assert!(body.contains("\"traceId\":\"unavailable\""));
        assert!(!body.contains("database password leaked"));
    }
}
