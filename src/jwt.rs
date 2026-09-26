use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: i64,
    exp: i64,
    iat: i64,
}

impl JwtClaims {
    pub fn new(id : i64, ttl : u32) -> Self {
        let iat = Utc::now();
        let exp = iat + Duration::try_seconds(ttl as i64).expect("ttl overflow");

        Self { sub: id, exp: exp.timestamp(), iat: iat.timestamp() }
    }
}

pub fn sign(id: i64, ttl: u32, encoding_key: &EncodingKey) -> Result<String, jsonwebtoken::errors::Error> {
    jsonwebtoken::encode(&Header::default(), &JwtClaims::new(id, ttl), encoding_key)
}

pub fn validate(token: &str, decoding_key: &DecodingKey) -> Result<JwtClaims, jsonwebtoken::errors::Error> {
    let data = jsonwebtoken::decode(token, decoding_key, &Validation::default())?;
    Ok(data.claims)
}