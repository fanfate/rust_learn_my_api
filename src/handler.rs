use crate::auth::{self, UserLogin, UserRegister};
use crate::error::ApiError;
use crate::models::{LoginResponse, UpdateUserRequest, UserResponse};
use crate::response::ApiResponse;
use crate::sql;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde_json::json;

pub async fn health() -> Json<serde_json::Value> {
    Json(json!({"status" : "ok"}))
}

pub async fn get_user_id(
    Path(user_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<UserResponse>, ApiError> {
    let user = sql::get_user_by_id(&state.pool, user_id)
        .await
        .map_err(ApiError::Sql)?;
    if let Some(user) = user {
        Ok(ApiResponse::success(user.into()))
    } else {
        Err(ApiError::NotFound("用户不存在".to_string()))
    }
}

pub async fn put_user_id(
    Path(user_id): Path<i64>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<ApiResponse<UserResponse>, ApiError> {
    let user = sql::get_user_by_id(&state.pool, user_id)
        .await
        .map_err(ApiError::Sql)?;
    if let Some(user) = user {
        let mut user: UserResponse = user.into();
        let mut has_change = false;
        if let Some(name) = payload.name {
            user.name = name;
            has_change = true;
        }
        if let Some(message) = payload.message {
            user.message = Some(message);
            has_change = true;
        }

        if has_change {
            let res = sql::update_user(&state.pool, user_id, &user.name, &user.message.as_deref())
                .await
                .map_err(ApiError::Sql)?;
            if !res {
                Err(ApiError::NotFound("用户不存在".to_string()))
            } else {
                Ok(ApiResponse::success(user))
            }
        } else {
            Err(ApiError::BadRequest("请输入要修改的参数".to_string()))
        }
    } else {
        Err(ApiError::NotFound("用户不存在".to_string()))
    }
}

pub async fn delete_user_id(
    Path(user_id): Path<i64>,
    State(state): State<AppState>,
) -> Result<ApiResponse<()>, ApiError> {
    let res = sql::delete_user(&state.pool, user_id)
        .await
        .map_err(ApiError::Sql)?;
    if !res {
        Err(ApiError::NotFound("用户不存在".to_string()))
    } else {
        Ok(ApiResponse::success_empty())
    }
}

pub async fn create_user(
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
