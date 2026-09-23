mod config;
mod db;
mod error;
mod handler;
mod models;
mod response;
mod sql;

use axum::{Router, routing::get};
use config::Config;

#[tokio::main]
async fn main() {
    let cfg = Config::load();

    let db_pool = db::create_pool(&cfg.database_url).await.unwrap();
    sql::init_db(&db_pool).await.unwrap();

    let app = Router::new()
        .route("/health", get(handler::health))
        .route(
            "/users/{id}",
            get(handler::get_user_id)
                .put(handler::put_user_id)
                .delete(handler::delete_user_id),
        )
        .route(
            "/users",
            get(handler::get_user_query).post(handler::create_user),
        )
        .with_state(db_pool);

    let listener = tokio::net::TcpListener::bind(&cfg.server_addr)
        .await
        .unwrap();

    println!("Server running on {0}", &cfg.server_addr);

    axum::serve(listener, app).await.unwrap();
}
