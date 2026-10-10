use crate::models::{Role, UserEntity};
use sqlx::SqlitePool;

pub async fn get_all_users(pool: &SqlitePool) -> Result<Vec<UserEntity>, sqlx::Error> {
    let users = sqlx::query_as!(
        UserEntity,
        "SELECT id, role as 'role: Role', name, email, password_hash, message FROM users"
    )
    .fetch_all(pool)
    .await?;

    Ok(users)
}

pub async fn get_user_by_id(pool: &SqlitePool, id: i64) -> Result<Option<UserEntity>, sqlx::Error> {
    let user = sqlx::query_as!(
        UserEntity,
        "SELECT id, role as 'role: Role', name, email, password_hash, message FROM users WHERE id = ?",
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn get_user_by_email(
    pool: &SqlitePool,
    email: &str,
) -> Result<Option<UserEntity>, sqlx::Error> {
    let user = sqlx::query_as!(
        UserEntity,
        "SELECT id, role as 'role: Role', name, email, password_hash, message FROM users WHERE email = ?",
        email
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn create_user(
    pool: &SqlitePool,
    name: &str,
    email: &str,
    password_hash: &str,
) -> Result<i64, sqlx::Error> {
    let res = sqlx::query!(
        "INSERT INTO users (name, email, password_hash) VALUES (?,?,?)",
        name,
        email,
        password_hash
    )
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

pub async fn delete_user(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let res = sqlx::query!("DELETE FROM users WHERE id = ?", id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn update_user(
    pool: &SqlitePool,
    id: i64,
    name: &str,
    message: &Option<&str>,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query!(
        "UPDATE users SET name = ?, message = COALESCE(?,message) WHERE id = ?",
        name,
        message,
        id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn update_user_password(
    pool: &SqlitePool,
    id: i64,
    password_hash: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query!(
        "UPDATE users SET password_hash = ? WHERE id = ?",
        password_hash,
        id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
