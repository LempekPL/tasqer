mod v1;

use anyhow::Context;
use axum::Router;
use sqlx::{PgPool, Pool, Postgres};

pub fn app(db: PgPool) -> Router {
    Router::new()
        .merge(v1::router())
        .with_state(db)
}

pub async fn run(db: PgPool) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3010").await?;
    axum::serve(listener, app(db))
        .await
        .context("failed to serve API")
}

