mod user;

use crate::api::v1::user::{get_self, login, logout};
use rkt::serde::json::Json;
use rkt::{Route, State, post};
use serde::Deserialize;
use sqlx::PgPool;

pub fn routes() -> Vec<Route> {
    rkt::routes![login, get_self, logout, create_task]
}

#[derive(Deserialize)]
struct CreateTaskPayload {
    title: String,
    desc: Option<String>,
}

#[post("/create", data = "<data>")]
async fn create_task(pool: &State<PgPool>, data: Json<CreateTaskPayload>) -> String {
    let stri: String = sqlx::query_scalar("SELECT 'hello world from pg'")
        .fetch_one(pool.inner())
        .await
        .unwrap();

    let response_title = format!("{} {}", stri, data.title);
    response_title
}
