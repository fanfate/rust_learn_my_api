use sqlx::SqlitePool;
use tower::ServiceExt;

mod common;

#[sqlx::test]
async fn test_health(pool: SqlitePool) {
    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/health")
        .body(axum::body::Body::empty())
        .unwrap();

    let app_state = common::test_state(pool);

    let resp = my_api::app(app_state).oneshot(req).await.unwrap();

    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}

#[sqlx::test]
async fn test_register(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    let register_body = serde_json::json!({
        "name": "alice",
        "email": "alice@local",
        "password": "user12345"
    })
    .to_string();

    let register_req = axum::http::Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(register_body))
        .unwrap();

    let register_resp = my_api::app(app_state).oneshot(register_req).await.unwrap();

    assert_eq!(register_resp.status(), axum::http::StatusCode::CREATED);
}
