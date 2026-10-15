#![allow(dead_code)]

use axum::{
    body::{Body, Bytes},
    http::Request,
};
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

pub async fn send(state: &AppState, req: Request<Body>) -> (axum::http::StatusCode, Bytes) {
    let resp = my_api::app(state.clone()).oneshot(req).await.unwrap();

    (
        resp.status(),
        axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
}

pub async fn register_ok(state: &AppState, name: &str, email: &str, password: &str) {
    let register_body = serde_json::json!({
        "name": name,
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

    let (status_code, _) = send(state, register_req).await;
    assert_eq!(status_code, axum::http::StatusCode::CREATED);
}

pub async fn login_ok(state: &AppState, email: &str, password: &str) -> (i64, String) {
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

    let (status_code, body_bytes) = send(state, login_req).await;
    assert_eq!(status_code, axum::http::StatusCode::OK);
    let login_envelope: LoginEnvelope = serde_json::from_slice(&body_bytes).unwrap();

    (login_envelope.data.user.id, login_envelope.data.token)
}

pub async fn register_and_login_ok(
    state: &AppState,
    name: &str,
    email: &str,
    password: &str,
) -> (i64, String) {
    register_ok(state, name, email, password).await;
    login_ok(state, email, password).await
}

pub async fn update_admin_email(state: &AppState, email: &str) {
    sqlx::query!("UPDATE users SET role='admin' WHERE email = ?", email)
        .execute(&state.pool)
        .await
        .unwrap();
}
