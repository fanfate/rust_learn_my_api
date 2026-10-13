#![allow(dead_code)]

use chrono::Duration;
use jsonwebtoken::{DecodingKey, EncodingKey};
use my_api::state::AppState;
use sqlx::SqlitePool;
use tower::ServiceExt;

#[derive(serde::Deserialize)]
struct LoginEnvelope {
    data: LoginData,
}

#[derive(serde::Deserialize)]
struct LoginData {
    token: String,
    user: UserId,
}

#[derive(serde::Deserialize)]
struct UserId {
    id: i64,
}

pub fn test_state(pool: SqlitePool) -> AppState {
    let access_ttl = Duration::try_seconds(10).unwrap();
    let encoding_key = EncodingKey::from_secret("test".as_bytes());
    let decoding_key = DecodingKey::from_secret("test".as_bytes());

    AppState {
        pool,
        access_ttl,
        encoding_key,
        decoding_key,
    }
}

pub async fn register_and_login(state: &AppState, email: &str, password: &str) -> (i64, String) {
    let register_body = serde_json::json!({
        "name": "alice",
        "email": email,
        "password": password
    })
    .to_string();

    let register_req = axum::http::Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(register_body))
        .unwrap();

    let register_resp = my_api::app(state.clone())
        .oneshot(register_req)
        .await
        .unwrap();

    assert_eq!(register_resp.status(), axum::http::StatusCode::CREATED);

    let login_body = serde_json::json!({
        "email" : email,
        "password" : password
    })
    .to_string();

    let login_req = axum::http::Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(login_body))
        .unwrap();

    let login_resp = my_api::app(state.clone()).oneshot(login_req).await.unwrap();

    assert_eq!(login_resp.status(), axum::http::StatusCode::OK);
    let bytes = axum::body::to_bytes(login_resp.into_body(), usize::MAX)
        .await
        .unwrap();

    let login_envelope: LoginEnvelope = serde_json::from_slice(&bytes).unwrap();

    (login_envelope.data.user.id, login_envelope.data.token)
}
