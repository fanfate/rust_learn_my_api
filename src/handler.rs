use crate::db;
use crate::error::ApiError;
use crate::models::{CreateUserRequest, QueryRequest, QueryResponse, UpdateUserRequest, User};
use crate::response::ApiResponse;
use crate::sql;
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
    State(db): State<db::Db>,
) -> Result<ApiResponse<User>, ApiError> {
    let user = sql::get_user_by_id(&db, user_id)
        .await
        .map_err(ApiError::Sql)?;
    if let Some(user) = user {
        Ok(ApiResponse::success(user.clone()))
    } else {
        Err(ApiError::NotFound("用户不存在".to_string()))
    }
}

pub async fn put_user_id(
    Path(user_id): Path<i64>,
    State(db): State<db::Db>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<ApiResponse<User>, ApiError> {
    let mut user = sql::get_user_by_id(&db, user_id)
        .await
        .map_err(ApiError::Sql)?;
    if let Some(user) = user.as_mut() {
        let mut has_change = false;
        if let Some(name) = payload.name {
            user.name = name;
            has_change = true;
        }
        if let Some(email) = payload.email {
            user.email = email;
            has_change = true;
        }

        if has_change {
            let res = sql::update_user(&db, user_id, &user.name, &user.email)
                .await
                .map_err(ApiError::Sql)?;
            if !res {
                Err(ApiError::NotFound("用户不存在".to_string()))
            } else {
                Ok(ApiResponse::success(user.clone()))
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
    State(db): State<db::Db>,
) -> Result<ApiResponse<User>, ApiError> {
    let res = sql::delete_user(&db, user_id)
        .await
        .map_err(ApiError::Sql)?;
    if !res {
        Err(ApiError::NotFound("用户不存在".to_string()))
    } else {
        Ok(ApiResponse::success_empty())
    }
}

pub async fn get_user_query(
    Query(query): Query<QueryRequest>,
) -> Result<ApiResponse<QueryResponse>, ApiError> {
    Ok(ApiResponse::success(QueryResponse {
        keyword: query.keyword,
        page: query.page,
        size: query.size,
    }))
}

pub async fn create_user(
    State(db): State<db::Db>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<ApiResponse<User>, ApiError> {
    let id = sql::create_user(&db, &payload.name, &payload.email)
        .await
        .map_err(ApiError::Sql)?;
    let user = sql::get_user_by_id(&db, id).await.map_err(ApiError::Sql)?;
    if let Some(user) = user {
        Ok(ApiResponse::success(user))
    } else {
        Err(ApiError::NotFound("新增失败".to_string()))
    }
}
