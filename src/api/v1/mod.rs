mod user;

use crate::api::v1::user::user_login;
use rkt::serde::json::Json;
use rkt::{Route, State, post};
use serde::Deserialize;
use sqlx::PgPool;

pub fn routes() -> Vec<Route> {
    rkt::routes![user_login, create_task]
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
