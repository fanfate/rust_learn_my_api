use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{extract::FromRequestParts, http::header::AUTHORIZATION};
use chrono::TimeDelta;
use jsonwebtoken::EncodingKey;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    error::ApiError,
    jwt,
    models::{LoginResponse, UserEntity},
    sql,
    state::AppState,
};

#[derive(Debug, Clone, Deserialize)]
pub struct UserRegister {
    pub name: String,
    pub email: String,
    password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserLogin {
    pub email: String,
    password: String,
}

#[derive(Debug, Clone, Copy)]
pub struct CurrentUser {
    pub id: i64,
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let tokens = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;

        let claims =
            jwt::validate(tokens, &state.decoding_key).map_err(|_| ApiError::Unauthorized)?;

        Ok(CurrentUser { id: claims.sub })
    }
}

impl UserRegister {
    pub fn hash_password(&self) -> Result<String, argon2::password_hash::Error> {
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(self.password.as_bytes())?.to_string();
        Ok(password_hash)
    }
}

impl UserLogin {
    pub fn verify_password(&self, hashed_password: &str) -> bool {
        let argon2 = Argon2::default();
        let Ok(hash) = PasswordHash::new(hashed_password) else {
            return false;
        };
        argon2
            .verify_password(self.password.as_bytes(), &hash)
            .is_ok()
    }
}

pub async fn register(pool: &SqlitePool, user_sign: &UserRegister) -> Result<UserEntity, ApiError> {
    let hashed_password = user_sign.hash_password().map_err(ApiError::Password)?;
    let user_id = sql::create_user(pool, &user_sign.name, &user_sign.email, &hashed_password)
        .await
        .map_err(|e| {
            if e.as_database_error()
                .is_some_and(|db| db.is_unique_violation())
            {
                ApiError::Conflict("邮箱已注册！".to_string())
            } else {
                ApiError::Sql(e)
            }
        })?;

    sql::get_user_by_id(pool, user_id)
        .await
        .map_err(ApiError::Sql)?
        .ok_or_else(|| ApiError::Internal("注册后查询失败".to_string()))
}

pub async fn login(
    pool: &SqlitePool,
    access_ttl: TimeDelta,
    encoding_key: &EncodingKey,
    user_login: &UserLogin,
) -> Result<LoginResponse, ApiError> {
    let user = sql::get_user_by_email(pool, &user_login.email)
        .await
        .map_err(ApiError::Sql)?
        .ok_or(ApiError::Unauthorized)?;
    if !user_login.verify_password(&user.password_hash) {
        return Err(ApiError::Unauthorized);
    }
    let token = jwt::sign(user.id, access_ttl, encoding_key)
        .map_err(|_| ApiError::Internal("签发token出错！".to_string()))?;

    Ok(LoginResponse {
        token,
        user: user.into(),
    })
}

