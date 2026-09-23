use crate::response::ApiResponse;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use sqlx;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("资源不存在：{0}")]
    NotFound(String),

    #[error("参数错误：{0}")]
    BadRequest(String),

    #[error("未授权")]
    Unauthorized,

    #[error("内部错误：{0}")]
    Internal(String),

    #[error("数据库错误：{0}")]
    Sql(sqlx::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status_code, code, message) = match &self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, 404, msg.as_str()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, 400, msg.as_str()),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, 401, "未授权"),
            ApiError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, 500, msg.as_str()),
            ApiError::Sql(_) => (StatusCode::INTERNAL_SERVER_ERROR, 500, "数据库错误"),
        };

        ApiResponse::<()>::error(status_code, code, message).into_response()
    }
}
