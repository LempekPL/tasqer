mod admin;
mod group;
mod user;

use rkt::serde::json::Json;
use rkt::{Route, State, post};
use serde::Deserialize;
use sqlx::PgPool;

pub fn routes() -> Vec<Route> {
    rkt::routes![
        user::register,
        user::login,
        user::get_self,
        user::logout,
        user::get_groups,
        group::create_group,
        group::get_groups,
        group::delete_group,
        group::join_group,
        group::add_to_group,
        admin::get_users,
        admin::delete_user,
        admin::create_invite,
        admin::get_groups,
        create_task
    ]
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
