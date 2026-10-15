use chrono::{TimeDelta, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::models::Role;

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: i64,
    pub role: Role,
    exp: i64,
    iat: i64,
}

impl JwtClaims {
    pub fn new(id: i64, role: &Role, access_ttl: TimeDelta) -> Self {
        let iat = Utc::now();
        let exp = iat + access_ttl;

        Self {
            sub: id,
            role: role.clone(),
            exp: exp.timestamp(),
            iat: iat.timestamp(),
        }
    }
}

pub fn sign(
    id: i64,
    role: &Role,
    access_ttl: TimeDelta,
    encoding_key: &EncodingKey,
) -> Result<String, jsonwebtoken::errors::Error> {
    jsonwebtoken::encode(
        &Header::default(),
        &JwtClaims::new(id, role, access_ttl),
        encoding_key,
    )
}

pub fn validate(
    token: &str,
    decoding_key: &DecodingKey,
) -> Result<JwtClaims, jsonwebtoken::errors::Error> {
    let data = jsonwebtoken::decode(token, decoding_key, &Validation::default())?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_sign_success() {
        let id = 1;
        let access_ttl = TimeDelta::try_seconds(10).unwrap();
        let encoding_key = EncodingKey::from_secret("test".as_bytes());

        let token = sign(id, &Role::User, access_ttl, &encoding_key);
        assert!(token.is_ok());
    }

    #[test]
    fn test_validate_success() {
        let id = 1;
        let access_ttl = TimeDelta::try_seconds(10).unwrap();
        let encoding_key = EncodingKey::from_secret("test".as_bytes());
        let decoding_key = DecodingKey::from_secret("test".as_bytes());

        let token = sign(id, &Role::User, access_ttl, &encoding_key).unwrap();

        let verified_claims = validate(&token, &decoding_key).unwrap();
        assert_eq!(verified_claims.sub, 1);
        assert_eq!(verified_claims.exp - verified_claims.iat, 10);
    }

    #[test]
    fn test_validate_wrong_format_token() {
        let decoding_key = DecodingKey::from_secret("test_another".as_bytes());
        let token = "wrong_token";

        let validate_err = validate(token, &decoding_key);

        match validate_err.unwrap_err().kind() {
            jsonwebtoken::errors::ErrorKind::InvalidToken => {}
            other => panic!("期望 InvalidToken, 实际错误 {:?}", other),
        }
    }

    #[test]
    fn test_validate_none_alg_token() {
        let decoding_key = DecodingKey::from_secret("test".as_bytes());
        let token = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.eyJzdWIiOjEsImV4cCI6MjAwMDAwMDAwMCwiaWF0IjoxNzAwMDAwMDAwfQ.";

        let validate_err = validate(token, &decoding_key);

        assert!(validate_err.is_err());
    }

    #[test]
    fn test_validate_other_algorithm_token() {
        let access_ttl = TimeDelta::try_seconds(10).unwrap();
        let encoding_key = EncodingKey::from_secret("test".as_bytes());
        let decoding_key = DecodingKey::from_secret("test".as_bytes());
        let token = jsonwebtoken::encode(
            &Header::new(jsonwebtoken::Algorithm::HS512),
            &JwtClaims::new(1, &Role::User, access_ttl),
            &encoding_key,
        )
        .unwrap();

        let validate_err = validate(&token, &decoding_key);

        match validate_err.unwrap_err().kind() {
            jsonwebtoken::errors::ErrorKind::InvalidAlgorithm => {}
            other => panic!("期望 InvalidAlgorithm, 实际错误 {:?}", other),
        }
    }

    #[test]
    fn test_validate_wrong_secret() {
        let id = 1;
        let access_ttl = TimeDelta::try_seconds(10).unwrap();
        let encoding_key = EncodingKey::from_secret("test".as_bytes());
        let decoding_key = DecodingKey::from_secret("test_another".as_bytes());
        let token = sign(id, &Role::User, access_ttl, &encoding_key).unwrap();

        let validate_err = validate(&token, &decoding_key);

        match validate_err.unwrap_err().kind() {
            jsonwebtoken::errors::ErrorKind::InvalidSignature => {}
            other => panic!("期望 InvalidSignature, 实际错误 {:?}", other),
        }
    }

    #[test]
    fn test_validate_time_failed() {
        let id = 1;
        let access_ttl = TimeDelta::try_seconds(-120).unwrap();
        let encoding_key = EncodingKey::from_secret("test".as_bytes());
        let decoding_key = DecodingKey::from_secret("test".as_bytes());
        let token = sign(id, &Role::User, access_ttl, &encoding_key).unwrap();

        let validate_err = validate(&token, &decoding_key);

        match validate_err.unwrap_err().kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {}
            other => panic!("期望 ExpiredSignature, 实际错误 {:?}", other),
        }
    }
}
