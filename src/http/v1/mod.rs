use crate::http::AppState;
use axum::extract::{Path, State};
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::PgPool;
use tasqer_macros::{get, routes};

pub fn router() -> Router<AppState> {
    Router::new().nest("/task", routes![create_task()])
}

#[derive(Deserialize)]
struct CreateTaskPayload {
    title: String,
    desc: Option<String>,
}

#[get("/create")]
pub async fn create_task(
    State(pool): State<PgPool>,
    Json(data): Json<CreateTaskPayload>,
) -> String {
    let stri: String = sqlx::query_scalar("select 'hello world from pg'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let title = stri + &data.title;
    title
    // "sss".to_string()
}
