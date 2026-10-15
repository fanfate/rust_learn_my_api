use sqlx::SqlitePool;

mod common;

#[sqlx::test]
async fn test_health(pool: SqlitePool) {
    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/health")
        .body(axum::body::Body::empty())
        .unwrap();

    let app_state = common::test_state(pool);
    let (status_code, _) = common::send(&app_state, req).await;
    assert_eq!(status_code, axum::http::StatusCode::OK);
}

#[sqlx::test]
async fn test_err_me(pool: SqlitePool) {
    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/me")
        .body(axum::body::Body::empty())
        .unwrap();

    let app_state = common::test_state(pool);
    let (status_code, _) = common::send(&app_state, req).await;
    assert_eq!(status_code, axum::http::StatusCode::UNAUTHORIZED);
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

    let (status_code, resp_bytes) = common::send(&app_state, register_req).await;
    let resp_v: serde_json::Value = serde_json::from_slice(&resp_bytes).unwrap();

    assert_eq!(status_code, axum::http::StatusCode::CREATED);
    assert_eq!(resp_v["data"]["name"], "alice");
    assert_eq!(resp_v["data"]["email"], "alice@local");
    assert_eq!(resp_v["data"]["role"], "user");
    assert_ne!(resp_v["data"].get("id"), None);
}

#[sqlx::test]
async fn test_register_twice(pool: SqlitePool) {
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

    let (status_code, _) = common::send(&app_state, register_req).await;
    assert_eq!(status_code, axum::http::StatusCode::CREATED);

    // same email
    let register_body2 = serde_json::json!({
        "name": "alice_2",
        "email": "alice@local",
        "password": "user12345"
    })
    .to_string();

    let register_req2 = axum::http::Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(register_body2))
        .unwrap();

    let (status_code2, _) = common::send(&app_state, register_req2).await;
    assert_eq!(status_code2, axum::http::StatusCode::CONFLICT);
}

#[sqlx::test]
async fn test_register_bad_syntax(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    let register_req = axum::http::Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, _) = common::send(&app_state, register_req).await;

    assert_eq!(status_code, axum::http::StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn test_register_missing_params(pool: SqlitePool) {
    let app_state = common::test_state(pool);
    let register_body = serde_json::json!({
        "name": "alice",
        "email": "alice@local"
    })
    .to_string();

    let register_req = axum::http::Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(register_body))
        .unwrap();

    let (status_code, _) = common::send(&app_state, register_req).await;

    assert_eq!(status_code, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn test_login_success(pool: SqlitePool) {
    let app_state = common::test_state(pool);
    common::register_and_login_ok(&app_state, "alice", "alice@local", "user12345").await;
}

#[sqlx::test]
async fn test_login_failed(pool: SqlitePool) {
    let app_state = common::test_state(pool);
    common::register_ok(&app_state, "alice", "alice@local", "user12345").await;

    let login_body = serde_json::json!({
        "email" : "alice@local",
        "password" : "12345"
    })
    .to_string();

    let login_req = axum::http::Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(login_body))
        .unwrap();

    let (status_code, _) = common::send(&app_state, login_req).await;
    assert_eq!(status_code, axum::http::StatusCode::UNAUTHORIZED);

    let login_body = serde_json::json!({
        "email" : "alice@local2",
        "password" : "user12345"
    })
    .to_string();

    let login_req = axum::http::Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(login_body))
        .unwrap();

    let (status_code, _) = common::send(&app_state, login_req).await;
    assert_eq!(status_code, axum::http::StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_admin_access_failed(pool: SqlitePool) {
    let app_state = common::test_state(pool);
    let get_users_req = axum::http::Request::builder()
        .method("GET")
        .uri("/admin/users")
        .header("content-type", "application/json")
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, _) = common::send(&app_state, get_users_req).await;
    assert_eq!(status_code, axum::http::StatusCode::UNAUTHORIZED);

    let (_, token) =
        common::register_and_login_ok(&app_state, "alice", "alice@local", "user12345").await;

    let get_users_req = axum::http::Request::builder()
        .method("GET")
        .uri("/admin/users")
        .header("authorization", format!("Bearer {}", token))
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, _) = common::send(&app_state, get_users_req).await;
    assert_eq!(status_code, axum::http::StatusCode::FORBIDDEN);
}

#[sqlx::test]
async fn test_admin_access_success(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    common::register_ok(&app_state, "alice", "alice@local", "user12345").await;
    common::update_admin_email(&app_state, "alice@local").await;

    let (_, token) = common::login_ok(&app_state, "alice@local", "user12345").await;

    let get_users_req = axum::http::Request::builder()
        .method("GET")
        .uri("/admin/users")
        .header("authorization", format!("Bearer {}", token))
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, _) = common::send(&app_state, get_users_req).await;
    assert_eq!(status_code, axum::http::StatusCode::OK);
}

#[sqlx::test]
async fn test_admin_other(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    common::register_ok(&app_state, "alice", "alice@local", "user12345").await;
    common::update_admin_email(&app_state, "alice@local").await;

    let (_, token) = common::login_ok(&app_state, "alice@local", "user12345").await;

    let get_other_req = axum::http::Request::builder()
        .method("GET")
        .uri("/admin/others")
        .header("authorization", format!("Bearer {}", token))
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, _) = common::send(&app_state, get_other_req).await;
    assert_eq!(status_code, axum::http::StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn test_get_user(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    let (id, _) =
        common::register_and_login_ok(&app_state, "alice", "alice@local", "user12345").await;

    let get_users_req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/users/{}", id))
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, body_bytes) = common::send(&app_state, get_users_req).await;
    assert_eq!(status_code, axum::http::StatusCode::OK);
    let body_value: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_value["data"]["name"], "alice");
    assert_eq!(body_value.get("email"), None);
}

#[sqlx::test]
async fn test_get_me_success(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    let (_, token) =
        common::register_and_login_ok(&app_state, "alice", "alice@local", "user12345").await;

    let get_me_req = axum::http::Request::builder()
        .method("GET")
        .uri("/me")
        .header("authorization", format!("Bearer {}", token))
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, body_bytes) = common::send(&app_state, get_me_req).await;
    assert_eq!(status_code, axum::http::StatusCode::OK);

    let body_value: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_value["data"]["name"], "alice");
    assert_eq!(body_value["data"]["email"], "alice@local");
    assert_eq!(body_value["data"]["message"], serde_json::Value::Null);
}

#[sqlx::test]
async fn test_patch_me_name_only(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    let (_, token) =
        common::register_and_login_ok(&app_state, "alice", "alice@local", "user12345").await;

    let patch_body = serde_json::json!({
        "name" : "bob",
    })
    .to_string();
    let patch_me_req = axum::http::Request::builder()
        .method("PATCH")
        .uri("/me")
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(patch_body))
        .unwrap();

    let (status_code, body_bytes) = common::send(&app_state, patch_me_req).await;
    assert_eq!(status_code, axum::http::StatusCode::OK);

    let body_value: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_value["data"]["name"], "bob");
    assert_eq!(body_value["data"]["email"], "alice@local");
    assert_eq!(body_value["data"]["message"], serde_json::Value::Null);
}

#[sqlx::test]
async fn test_change_pwd_wrong_old(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    let (_, token) =
        common::register_and_login_ok(&app_state, "alice", "alice@local", "user12345").await;

    let patch_body = serde_json::json!({
        "old_password" : "user123456789",
        "new_password" : "user1234567",
    })
    .to_string();
    let patch_me_req = axum::http::Request::builder()
        .method("PUT")
        .uri("/me/password")
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(patch_body))
        .unwrap();

    let (status_code, _) = common::send(&app_state, patch_me_req).await;
    assert_eq!(status_code, axum::http::StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn test_change_pwd_success(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    let (_, token) =
        common::register_and_login_ok(&app_state, "alice", "alice@local", "user12345").await;

    let patch_body = serde_json::json!({
        "old_password" : "user12345",
        "new_password" : "user123456789",
    })
    .to_string();
    let patch_me_req = axum::http::Request::builder()
        .method("PUT")
        .uri("/me/password")
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(patch_body))
        .unwrap();

    let (status_code, _) = common::send(&app_state, patch_me_req).await;
    assert_eq!(status_code, axum::http::StatusCode::OK);
}

#[sqlx::test]
async fn test_delete_me_then_404(pool: SqlitePool) {
    let app_state = common::test_state(pool);

    let (id, token) =
        common::register_and_login_ok(&app_state, "alice", "alice@local", "user12345").await;

    let delete_me_req = axum::http::Request::builder()
        .method("DELETE")
        .uri("/me")
        .header("authorization", format!("Bearer {}", token))
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, _) = common::send(&app_state, delete_me_req).await;
    assert_eq!(status_code, axum::http::StatusCode::OK);

    let login_body = serde_json::json!({
        "email" : "alice@local",
        "password" : "user12345"
    })
    .to_string();

    let login_req = axum::http::Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(login_body))
        .unwrap();

    let (status_code, _) = common::send(&app_state, login_req).await;
    assert_eq!(status_code, axum::http::StatusCode::UNAUTHORIZED);

    let get_user_req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/users/{}", id))
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, _) = common::send(&app_state, get_user_req).await;
    assert_eq!(status_code, axum::http::StatusCode::NOT_FOUND);

    let get_me_req = axum::http::Request::builder()
        .method("GET")
        .uri("/me")
        .header("authorization", format!("Bearer {}", token))
        .body(axum::body::Body::empty())
        .unwrap();

    let (status_code, _) = common::send(&app_state, get_me_req).await;
    assert_eq!(status_code, axum::http::StatusCode::NOT_FOUND);
}
