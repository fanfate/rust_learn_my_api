use jsonwebtoken::{DecodingKey, EncodingKey};

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
}