use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{extract::FromRequestParts, http::header::AUTHORIZATION};
use chrono::TimeDelta;
use jsonwebtoken::EncodingKey;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    error::ApiError,
    jwt,
    models::{LoginResponse, UpdatePasswordRequest, UpdateUserRequest, UserEntity},
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

fn verify_password(origin_password: &str, hashed_password: &str) -> bool {
    let argon2 = Argon2::default();
    let Ok(hash) = PasswordHash::new(hashed_password) else {
        return false;
    };
    argon2
        .verify_password(origin_password.as_bytes(), &hash)
        .is_ok()
}

fn hash_password(origin_password: &str) -> Result<String, argon2::password_hash::Error> {
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(origin_password.as_bytes())?
        .to_string();
    Ok(password_hash)
}

// services

pub async fn register(pool: &SqlitePool, user_sign: &UserRegister) -> Result<UserEntity, ApiError> {
    let hashed_password = hash_password(&user_sign.password).map_err(ApiError::Password)?;
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
    if !verify_password(&user_login.password, &user.password_hash) {
        return Err(ApiError::Unauthorized);
    }
    let token = jwt::sign(user.id, access_ttl, encoding_key)
        .map_err(|_| ApiError::Internal("签发token出错！".to_string()))?;

    Ok(LoginResponse {
        token,
        user: user.into(),
    })
}

pub async fn update_me(
    pool: &SqlitePool,
    user_id: i64,
    user_update: UpdateUserRequest,
) -> Result<UserEntity, ApiError> {
    let mut user = get_user(pool, user_id).await?;

    let mut has_change = false;
    if let Some(name) = user_update.name {
        user.name = name;
        has_change = true;
    }
    if let Some(message) = user_update.message {
        user.message = Some(message);
        has_change = true;
    }

    if has_change {
        let res = sql::update_user(pool, user_id, &user.name, &user.message.as_deref())
            .await
            .map_err(ApiError::Sql)?;
        if !res {
            Err(ApiError::NotFound("用户不存在".to_string()))
        } else {
            Ok(user)
        }
    } else {
        Err(ApiError::BadRequest("请输入要修改的参数".to_string()))
    }
}

pub async fn delete_me(pool: &SqlitePool, user_id: i64) -> Result<(), ApiError> {
    let res = sql::delete_user(pool, user_id)
        .await
        .map_err(ApiError::Sql)?;
    if !res {
        Err(ApiError::NotFound("用户不存在".to_string()))
    } else {
        Ok(())
    }
}

pub async fn update_password_me(
    pool: &SqlitePool,
    user_id: i64,
    user_update: UpdatePasswordRequest,
) -> Result<(), ApiError> {
    let user = get_user(pool, user_id).await?;

    if !verify_password(&user_update.old_password, &user.password_hash) {
        return Err(ApiError::Unauthorized);
    }

    let new_password_hash = hash_password(&user_update.new_password).map_err(ApiError::Password)?;

    let update_res = sql::update_user_password(pool, user_id, &new_password_hash)
        .await
        .map_err(ApiError::Sql)?;

    if update_res {
        Ok(())
    } else {
        Err(ApiError::Internal("更新失败".to_string()))
    }
}

pub async fn get_user(pool: &SqlitePool, user_id: i64) -> Result<UserEntity, ApiError> {
    let user = sql::get_user_by_id(pool, user_id)
        .await
        .map_err(ApiError::Sql)?
        .ok_or(ApiError::NotFound("用户不存在".to_string()))?;
    Ok(user)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_hash_success() {
        let origin_pwd = "123_tmp";
        let hashed_pwd = hash_password(origin_pwd).unwrap();

        assert!(argon2::PasswordHash::new(&hashed_pwd).is_ok());
    }

    #[test]
    fn test_hash_salt() {
        let origin_pwd = "123tmp";
        let hashed_0 = hash_password(origin_pwd);
        let hashed_1 = hash_password(origin_pwd);

        assert_ne!(hashed_0, hashed_1);
    }

    #[test]
    fn test_verify_success() {
        let origin_pwd = "123tmp";
        let hashed_pwd = hash_password(origin_pwd).unwrap();

        let verified = verify_password(origin_pwd, &hashed_pwd);
        assert!(verified);
    }

    #[test]
    fn test_verify_failed() {
        let hashd_pwd = hash_password("123tmp").unwrap();
        let un_verified = verify_password("456tmp", &hashd_pwd);
        assert!(!un_verified);
    }

    #[test]
    fn test_verify_failed_error() {
        let error_verified = verify_password("123tmp", "error_hash");
        assert!(!error_verified);
    }
}
