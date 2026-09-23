use dotenvy::dotenv;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub server_addr: String,
}

impl Config {
    pub fn load() -> Self {
        dotenv().ok();
        Self {
            database_url: env::var("DATABASE_URL").unwrap_or("sqlite:data/users.db".to_string()),
            server_addr: env::var("SERVER_ADDR").unwrap_or("127.0.0.1:3000".to_string()),
        }
    }
}
