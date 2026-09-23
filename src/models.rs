use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct QueryResponse {
    pub keyword: String,
    pub page: Option<usize>,
    pub size: Option<usize>,
}

#[derive(Deserialize)]
pub struct QueryRequest {
    pub keyword: String,
    pub page: Option<usize>,
    pub size: Option<usize>,
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
}
