use anyhow::Context;
use axum::Router;
use axum::extract::{FromRef, Path};
use sqlx::PgPool;
use tasqer_macros::{get, post, routes};

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

#[post("/test")]
pub async fn test2() -> &'static str {
    "Hello, World!"
}

#[get("/test/{id}")]
pub async fn test(Path(id): Path<u64>) -> String {
    let id = id.to_string();
    id
    // "Hello, World!"
}

pub async fn run(db: PgPool) -> anyhow::Result<()> {
    let ap = Router::new()
        .merge(routes![test2(), test()])
        .nest("/v1", test())
        .with_state(AppState { pool: db });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3010").await?;
    axum::serve(listener, ap)
        .await
        .context("failed to serve API")
}
