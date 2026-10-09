use crate::auth::{self, CurrentUser, UserLogin, UserRegister};
use crate::error::ApiError;
use crate::models::{
    LoginResponse, PublicUserResponse, UpdatePasswordRequest, UpdateUserRequest, UserResponse,
};
use crate::response::ApiResponse;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::json;

pub async fn health() -> Json<serde_json::Value> {
    Json(json!({"status" : "ok"}))
}

pub async fn get_user_id(
    Path(user_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<PublicUserResponse>, ApiError> {
    let user = auth::get_user(&state.pool, user_id).await?;
    Ok(ApiResponse::success(user.into()))
}

// me
pub async fn get_me(
    current: CurrentUser,
    State(state): State<AppState>,
) -> Result<ApiResponse<UserResponse>, ApiError> {
    let user = auth::get_user(&state.pool, current.id).await?;
    Ok(ApiResponse::success(user.into()))
}

pub async fn change_me(
    current: CurrentUser,
    State(state): State<AppState>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<ApiResponse<UserResponse>, ApiError> {
    let user = auth::update_me(&state.pool, current.id, payload).await?;
    Ok(ApiResponse::success(user.into()))
}

pub async fn delete_me(
    current: CurrentUser,
    State(state): State<AppState>,
) -> Result<ApiResponse<()>, ApiError> {
    let _res = auth::delete_me(&state.pool, current.id).await?;
    Ok(ApiResponse::success_empty())
}

pub async fn update_password_me(
    current: CurrentUser,
    State(state): State<AppState>,
    Json(payload): Json<UpdatePasswordRequest>,
) -> Result<ApiResponse<()>, ApiError> {
    let _res = auth::update_password_me(&state.pool, current.id, payload).await?;
    Ok(ApiResponse::success_empty())
}

// auth

pub async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<UserRegister>,
) -> Result<ApiResponse<UserResponse>, ApiError> {
    let user = auth::register(&state.pool, &payload).await?;
    Ok(ApiResponse::created(user.into()))
}

pub async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<UserLogin>,
) -> Result<ApiResponse<LoginResponse>, ApiError> {
    let response =
        auth::login(&state.pool, state.access_ttl, &state.encoding_key, &payload).await?;
    Ok(ApiResponse::success(response))
}
