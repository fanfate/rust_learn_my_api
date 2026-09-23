use sqlx::SqlitePool;

pub type Db = SqlitePool;

pub async fn create_pool(database_url: &str) -> Result<Db, sqlx::Error> {
    let pool = SqlitePool::connect(database_url).await?;
    Ok(pool)
}
