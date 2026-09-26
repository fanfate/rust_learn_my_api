use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use serde::{Deserialize,};
use sqlx::SqlitePool;

use crate::{error::ApiError, sql};


#[derive(Debug, Clone, Deserialize)]
pub struct UserSign {
    pub name : String,
    pub email : String,
    password : String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserLogin {
    pub email : String,
    password : String,
}

impl UserSign {
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
        argon2.verify_password(self.password.as_bytes(), &hash).is_ok()
    }
}


pub async fn register(pool: &SqlitePool, user_sign : &UserSign) -> Result<i64, ApiError>{
    let hashed_password = user_sign.hash_password().map_err(ApiError::Password)?;    
    let user_id =  sql::create_user(pool, &user_sign.name, &user_sign.email, &hashed_password).await.map_err(ApiError::Sql)?;
    Ok(user_id)
}

pub async fn login(pool: &SqlitePool, user_login : &UserLogin) -> Result<i64, ApiError> {
    let user = sql::get_user_with_hash_by_email(pool, &user_login.email).await.map_err(ApiError::Sql)?.ok_or( ApiError::Unauthorized)?;
    if user_login.verify_password(&user.password_hash) {
        Ok(user.id)
    } else {
        Err(ApiError::Unauthorized)
    }
}