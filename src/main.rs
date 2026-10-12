mod http;
mod password;

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = dotenvy::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Database connection failed")?;

    sqlx::migrate!().run(&db).await?;

    http::run(db).await
}

