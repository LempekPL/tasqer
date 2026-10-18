use rkt::{post, State};
use rkt::serde::json::Json;
use sqlx::PgPool;
use crate::api::v1::CreateTaskPayload;

#[post("/user/login", data = "<data>")]
pub async fn user_login(pool: &State<PgPool>, data: Json<CreateTaskPayload>) -> String {
    let stri: String = sqlx::query_scalar("SELECT 'hello world from pg'")
        .fetch_one(pool.inner())
        .await
        .unwrap();

    let response_title = format!("{} {}", stri, data.title);
    response_title
}
