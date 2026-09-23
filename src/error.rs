use crate::response::ApiResponse;
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
        let (code, message) = match &self {
            ApiError::NotFound(msg) => (404, msg.as_str()),
            ApiError::BadRequest(msg) => (400, msg.as_str()),
            ApiError::Unauthorized => (401, "未授权"),
            ApiError::Internal(msg) => (500, msg.as_str()),
            ApiError::Sql(_) => (500, "数据库错误"),
        };

        ApiResponse::<()>::error(code, message).into_response()
    }
}

// impl ApiError {
//     pub fn from_sqlx_error(err: &sqlx::Error) -> Self {
//         ApiError::Internal(format!("Internal Error : {:?}.", err))
//     }
// }
