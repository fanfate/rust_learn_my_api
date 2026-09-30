mod auth;
mod config;
mod db;
mod error;
mod handler;
mod jwt;
mod models;
mod response;
mod sql;
mod state;

use axum::{
    Router,
    routing::{get, post, put},
};
use chrono::Duration;
use config::Config;
use jsonwebtoken::{DecodingKey, EncodingKey};

use crate::state::AppState;

#[tokio::main]
async fn main() {
    let cfg = Config::load();

    let db_pool = db::create_pool(&cfg.database_url).await.unwrap();
    let access_ttl = Duration::try_seconds(cfg.jwt_access_ttl as i64).unwrap();
    let encoding_key = EncodingKey::from_secret(cfg.jwt_secret.as_ref());
    let decoding_key = DecodingKey::from_secret(cfg.jwt_secret.as_ref());

    let app_state = AppState {
        pool: db_pool,
        access_ttl,
        encoding_key,
        decoding_key,
    };

    let app = Router::new()
        .route("/health", get(handler::health))
        .route("/auth/register", post(handler::register_user))
        .route("/auth/login", post(handler::login_user))
        .route("/users/{id}", get(handler::get_user_id))
        .route(
            "/me",
            get(handler::get_me)
                .patch(handler::change_me)
                .delete(handler::delete_me),
        )
        .route("/me/password", put(handler::update_password_me))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&cfg.server_addr)
        .await
        .unwrap();

    println!("Server running on {0}", cfg.server_addr);

    axum::serve(listener, app).await.unwrap();
}
