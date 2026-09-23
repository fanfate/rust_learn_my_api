use crate::models::User;
use sqlx::SqlitePool;

pub async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            message TEXT
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_all_users(pool: &SqlitePool) -> Result<Vec<User>, sqlx::Error> {
    let users = sqlx::query_as!(User, "SELECT id, name, email, message FROM users")
        .fetch_all(pool)
        .await?;

    Ok(users)
}

pub async fn get_user_by_id(pool: &SqlitePool, id: i64) -> Result<Option<User>, sqlx::Error> {
    let user = sqlx::query_as!(
        User,
        "SELECT id, name, email, message FROM users WHERE id = ?",
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn create_user(pool: &SqlitePool, name: &str, email: &str) -> Result<i64, sqlx::Error> {
    let res = sqlx::query!("INSERT INTO users (name, email) VALUES (?,?)", name, email)
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
    email: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query!(
        "UPDATE users SET name = ?, email = ? WHERE id = ?",
        name,
        email,
        id
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
