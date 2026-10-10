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

fn is_valid_password(origin_password: &str) -> bool {
    origin_password.chars().count() >= 8
}

// services

pub async fn register(pool: &SqlitePool, user_sign: &UserRegister) -> Result<UserEntity, ApiError> {
    if !is_valid_password(&user_sign.password) {
        return Err(ApiError::BadRequest("密码长度不能小于8".to_string()));
    }

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

    if !is_valid_password(&user_update.new_password) {
        return Err(ApiError::BadRequest("密码长度不能小于8".to_string()));
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

    const TMP_SECRET: &str = "123456789_tmp_secret";

    use jsonwebtoken::DecodingKey;

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

    #[test]
    fn test_valid_password() {
        assert!(is_valid_password("123456ABCtmp"));
        assert!(!is_valid_password(""));
        assert!(!is_valid_password("123"));
    }

    #[sqlx::test]
    async fn test_register_success(pool: SqlitePool) {
        let user_sign = UserRegister {
            name: "name1".to_string(),
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };
        let user = register(&pool, &user_sign).await.expect("期望注册成功!");

        assert_ne!(user_sign.password, user.password_hash);
        assert!(verify_password(&user_sign.password, &user.password_hash));
        assert_eq!(user_sign.name, user.name);
        assert_eq!(user_sign.email, user.email);
    }

    #[sqlx::test]
    async fn test_register_same_email(pool: SqlitePool) {
        let user_sign_1 = UserRegister {
            name: "name1".to_string(),
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };

        let user_sign_2 = UserRegister {
            name: "name2".to_string(),
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };

        let _ = register(&pool, &user_sign_1).await.expect("期望注册成功!");

        let same_email_err = register(&pool, &user_sign_2).await;

        assert!(matches!(same_email_err, Err(ApiError::Conflict(_))));
    }

    #[sqlx::test]
    async fn test_register_short_password(pool: SqlitePool) {
        let user_sign = UserRegister {
            name: "name1".to_string(),
            email: "test@test.email".to_string(),
            password: "".to_string(),
        };

        let empty_pwd_err = register(&pool, &user_sign).await;

        assert!(matches!(empty_pwd_err, Err(ApiError::BadRequest(_))));

        let user_sign = UserRegister {
            name: "name1".to_string(),
            email: "test@test.email".to_string(),
            password: "123".to_string(),
        };

        let short_pwd_err = register(&pool, &user_sign).await;

        assert!(matches!(short_pwd_err, Err(ApiError::BadRequest(_))));
    }

    #[sqlx::test]
    async fn test_login_success(pool: SqlitePool) {
        // register
        let user_sign = UserRegister {
            name: "name1".to_string(),
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };
        let user = register(&pool, &user_sign).await.expect("期望注册成功!");

        let user_login = UserLogin {
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };

        let access_ttl = TimeDelta::try_seconds(10).unwrap();
        let encoding_key = EncodingKey::from_secret(TMP_SECRET.as_bytes());
        let decoding_key = DecodingKey::from_secret(TMP_SECRET.as_bytes());

        let login_res = login(&pool, access_ttl, &encoding_key, &user_login)
            .await
            .expect("期望登录成功");

        assert_eq!(user.id, login_res.user.id);
        assert_eq!(user.name, login_res.user.name);
        assert_eq!(user.email, login_res.user.email);

        let token_id = jwt::validate(&login_res.token, &decoding_key).unwrap().sub;

        assert_eq!(user.id, token_id);
    }

    #[sqlx::test]
    async fn test_login_failed(pool: SqlitePool) {
        // register
        let user_sign = UserRegister {
            name: "name1".to_string(),
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };
        let _ = register(&pool, &user_sign).await.expect("期望注册成功!");

        let access_ttl = TimeDelta::try_seconds(10).unwrap();
        let encoding_key = EncodingKey::from_secret(TMP_SECRET.as_bytes());

        let user_login_1 = UserLogin {
            email: "test2@test2.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };

        let login_res_1 = login(&pool, access_ttl, &encoding_key, &user_login_1).await;

        assert!(matches!(login_res_1, Err(ApiError::Unauthorized)));
        let user_login_2 = UserLogin {
            email: "test@test.email".to_string(),
            password: "123456ABCtmp123".to_string(),
        };

        let login_res_2 = login(&pool, access_ttl, &encoding_key, &user_login_2).await;

        assert!(matches!(login_res_2, Err(ApiError::Unauthorized)));
    }

    #[sqlx::test]
    async fn test_update(pool: SqlitePool) {
        // register
        let user_sign = UserRegister {
            name: "name1".to_string(),
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };
        let user = register(&pool, &user_sign).await.expect("期望注册成功!");

        let update_request_0 = UpdateUserRequest {
            name: None,
            message: Some("message_0".to_string()),
        };

        let update_res0 = update_me(&pool, user.id, update_request_0)
            .await
            .expect("期望更新成功");

        assert_eq!(update_res0.id, user.id);
        assert_eq!(update_res0.email, user.email);
        assert_eq!(update_res0.message, Some("message_0".to_string()));
        assert_eq!(update_res0.name, user.name);

        let update_request_1 = UpdateUserRequest {
            name: Some("name_update_1".to_string()),
            message: None,
        };

        let update_res1 = update_me(&pool, user.id, update_request_1)
            .await
            .expect("期望更新成功");

        assert_eq!(update_res1.id, user.id);
        assert_eq!(update_res1.email, user.email);
        assert_eq!(update_res1.message, Some("message_0".to_string()));
        assert_eq!(update_res1.name, "name_update_1");

        let update_request_2 = UpdateUserRequest {
            name: Some("name_update_2".to_string()),
            message: Some("message_2".to_string()),
        };

        let update_res2 = update_me(&pool, user.id, update_request_2)
            .await
            .expect("期望更新成功");

        assert_eq!(update_res2.id, user.id);
        assert_eq!(update_res2.email, user.email);
        assert_eq!(update_res2.message, Some("message_2".to_string()));
        assert_eq!(update_res2.name, "name_update_2");

        let update_request_3 = UpdateUserRequest {
            name: None,
            message: None,
        };

        let update_res3 = update_me(&pool, user.id, update_request_3).await;

        assert!(matches!(update_res3, Err(ApiError::BadRequest(_))));

        let persist = get_user(&pool, user.id).await.expect("期望重查成功");

        assert_eq!(persist.id, user.id);
        assert_eq!(persist.email, user.email);
        assert_eq!(persist.password_hash, user.password_hash);
        assert_eq!(persist.name, "name_update_2");
        assert_eq!(persist.message, Some("message_2".to_string()));
    }

    #[sqlx::test]
    async fn test_update_password(pool: SqlitePool) {
        // register
        let user_sign = UserRegister {
            name: "name1".to_string(),
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };
        let user = register(&pool, &user_sign).await.expect("期望注册成功!");

        let update_pwd_1 = UpdatePasswordRequest {
            old_password: "123456ABCtmp123".to_string(),
            new_password: "123456ABCtmp456".to_string(),
        };

        let pwd_res1 = update_password_me(&pool, user.id, update_pwd_1).await;

        assert!(matches!(pwd_res1, Err(ApiError::Unauthorized)));

        let update_pwd_order = UpdatePasswordRequest {
            old_password: "123456ABCtmp123".to_string(),
            new_password: "123".to_string(),
        };

        let pwd_res_order = update_password_me(&pool, user.id, update_pwd_order).await;

        assert!(matches!(pwd_res_order, Err(ApiError::Unauthorized)));

        let update_pwd2 = UpdatePasswordRequest {
            old_password: "123456ABCtmp".to_string(),
            new_password: "123".to_string(),
        };

        let pwd_res_2 = update_password_me(&pool, user.id, update_pwd2).await;

        assert!(matches!(pwd_res_2, Err(ApiError::BadRequest(_))));

        let update_pwd3 = UpdatePasswordRequest {
            old_password: "123456ABCtmp".to_string(),
            new_password: "123456ABCtmpABC".to_string(),
        };

        let _ = update_password_me(&pool, user.id, update_pwd3)
            .await
            .expect("期望修改密码成功");

        let user_login = UserLogin {
            email: "test@test.email".to_string(),
            password: "123456ABCtmpABC".to_string(),
        };

        let access_ttl = TimeDelta::try_seconds(10).unwrap();
        let encoding_key = EncodingKey::from_secret(TMP_SECRET.as_bytes());
        let decoding_key = DecodingKey::from_secret(TMP_SECRET.as_bytes());

        let login_res = login(&pool, access_ttl, &encoding_key, &user_login)
            .await
            .expect("期望登录成功");

        assert_eq!(user.id, login_res.user.id);
        assert_eq!(user.name, login_res.user.name);
        assert_eq!(user.email, login_res.user.email);

        let token_id = jwt::validate(&login_res.token, &decoding_key).unwrap().sub;

        assert_eq!(user.id, token_id);

        let user_login_failed = UserLogin {
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };

        let login_failed = login(&pool, access_ttl, &encoding_key, &user_login_failed).await;

        assert!(matches!(login_failed, Err(ApiError::Unauthorized)));
    }

    #[sqlx::test]
    async fn test_delete_me(pool: SqlitePool) {
        // register
        let user_sign = UserRegister {
            name: "name1".to_string(),
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };
        let user = register(&pool, &user_sign).await.expect("期望注册成功!");

        let user_login = UserLogin {
            email: "test@test.email".to_string(),
            password: "123456ABCtmp".to_string(),
        };

        let access_ttl = TimeDelta::try_seconds(10).unwrap();
        let encoding_key = EncodingKey::from_secret(TMP_SECRET.as_bytes());
        let decoding_key = DecodingKey::from_secret(TMP_SECRET.as_bytes());

        let login_res = login(&pool, access_ttl, &encoding_key, &user_login)
            .await
            .expect("期望登录成功");

        assert_eq!(user.id, login_res.user.id);
        assert_eq!(user.name, login_res.user.name);
        assert_eq!(user.email, login_res.user.email);

        let token_id = jwt::validate(&login_res.token, &decoding_key).unwrap().sub;
        assert_eq!(user.id, token_id);

        let _ = delete_me(&pool, user.id).await.expect("期望注销成功");

        let login_failed = login(&pool, access_ttl, &encoding_key, &user_login).await;

        assert!(matches!(login_failed, Err(ApiError::Unauthorized)));
        assert!(matches!(
            get_user(&pool, user.id).await,
            Err(ApiError::NotFound(_))
        ));

        assert!(matches!(
            delete_me(&pool, user.id).await,
            Err(ApiError::NotFound(_))
        ));
    }
}
