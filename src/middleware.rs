use axum::{extract::Request, middleware::Next, response::Response};

use crate::{auth::CurrentUser, error::ApiError, models::Role};

pub async fn require_admin(
    user: CurrentUser,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    match user.role {
        Role::User => Err(ApiError::Forbidden),
        Role::Admin => Ok(next.run(req).await),
    }
}
