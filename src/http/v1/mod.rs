use axum::extract::State;
use axum::http::StatusCode;
use axum::Router;
use axum::routing::get;
use sqlx::PgPool;

pub fn router() -> Router<PgPool> {
    Router::new()
        .route("/v1", get(using_connection_pool_extractor))
}



async fn using_connection_pool_extractor(
    State(pool): State<PgPool>,
) -> Result<String, (StatusCode, String)> {
    sqlx::query_scalar("select 'hello world from pg'")
        .fetch_one(&pool)
        .await
        .map_err(internal_error)
}

fn internal_error<E>(err: E) -> (StatusCode, String)
where
    E: std::error::Error,
{
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}