use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;

#[derive(Debug)]
pub struct ApiResponse<T> {
    status_code: StatusCode,
    body: ApiBody<T>,
}

#[derive(Debug, Serialize)]
struct ApiBody<T> {
    code: i32,
    message: String,
    data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            status_code: StatusCode::OK,
            body: ApiBody {
                code: 200,
                message: "success".to_string(),
                data: Some(data),
            },
        }
    }

    pub fn success_empty() -> Self {
        Self {
            status_code: StatusCode::OK,
            body: ApiBody {
                code: 200,
                message: "success".to_string(),
                data: None,
            },
        }
    }

    pub fn error(status_code: StatusCode, code: i32, message: &str) -> Self {
        Self {
            status_code,
            body: ApiBody {
                code,
                message: message.to_string(),
                data: None,
            },
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (self.status_code, Json(self.body)).into_response()
    }
}
