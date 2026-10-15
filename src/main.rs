use chrono::Duration;
use jsonwebtoken::{DecodingKey, EncodingKey};
use my_api::{config::Config, db::create_pool, state::AppState};

#[tokio::main]
async fn main() {
    let cfg = Config::load();

    let db_pool = create_pool(&cfg.database_url).await.unwrap();
    let access_ttl = Duration::try_seconds(cfg.jwt_access_ttl as i64).unwrap();
    let encoding_key = EncodingKey::from_secret(cfg.jwt_secret.as_ref());
    let decoding_key = DecodingKey::from_secret(cfg.jwt_secret.as_ref());

    let app_state = AppState {
        pool: db_pool,
        access_ttl,
        encoding_key,
        decoding_key,
    };

    let app = my_api::app(app_state);

    let listener = tokio::net::TcpListener::bind(&cfg.server_addr)
        .await
        .unwrap();

    println!("Server running on {0}", cfg.server_addr);

    axum::serve(listener, app).await.unwrap();
}
