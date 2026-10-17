use anyhow::Context;
use axum::extract::{FromRef, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use sqlx::PgPool;
use tasqer_macros::routes;

mod v1;

#[derive(Clone)]
pub struct AppState {
    pool: PgPool,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

pub async fn run(pool: PgPool) -> anyhow::Result<()> {
    let ap = Router::new()
        .nest("/v1", v1::router())
        .with_state(AppState { pool });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3010").await?;
    axum::serve(listener, ap)
        .await
        .context("failed to serve API")
}
