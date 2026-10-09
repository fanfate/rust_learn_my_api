use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct UserEntity {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub password_hash: String,
    pub message: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct UserResponse {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub message: Option<String>,
}

impl From<UserEntity> for UserResponse {
    fn from(value: UserEntity) -> Self {
        Self {
            id: value.id,
            name: value.name,
            email: value.email,
            message: value.message,
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct PublicUserResponse {
    pub id: i64,
    pub name: String,
    pub message: Option<String>,
}

impl From<UserEntity> for PublicUserResponse {
    fn from(value: UserEntity) -> Self {
        Self {
            id: value.id,
            name: value.name,
            message: value.message,
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Deserialize)]
pub struct UpdatePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}
