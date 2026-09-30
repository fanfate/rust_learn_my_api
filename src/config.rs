use dotenvy::dotenv;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub server_addr: String,
    pub jwt_secret: String,
    pub jwt_access_ttl: u32,
}

impl Config {
    pub fn load() -> Self {
        dotenv().ok();
        Self {
            database_url: env::var("DATABASE_URL").unwrap_or("sqlite:data/users.db".to_string()),
            server_addr: env::var("SERVER_ADDR").unwrap_or("127.0.0.1:3000".to_string()),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET 必须在 .env 中配置"),
            jwt_access_ttl: env::var("JWT_ACCESS_TTL")
                .unwrap_or("900".to_string())
                .parse::<u32>()
                .unwrap_or(900),
        }
    }
}
