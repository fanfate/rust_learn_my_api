use axum::{
    Router,
    routing::{get, post, put},
};

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod handler;
pub mod jwt;
pub mod models;
pub mod response;
pub mod sql;
pub mod state;

pub fn app(state: state::AppState) -> Router {
    Router::new()
        .route("/health", get(handler::health))
        .route("/auth/register", post(handler::register_user))
        .route("/auth/login", post(handler::login_user))
        .route("/users/{id}", get(handler::get_user_id))
        .route(
            "/me",
            get(handler::get_me)
                .patch(handler::change_me)
                .delete(handler::delete_me),
        )
        .route("/me/password", put(handler::update_password_me))
        .with_state(state)
}
