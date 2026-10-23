use crate::api::v1::user::AuthUser;
use crate::util::{ApiResponse, ApiResult};
use chrono::{DateTime, Utc};
use rkt::http::Status;
use rkt::serde::json::Json;
use rkt::{State, delete, get, post};
use serde::{Deserialize, Serialize};
use sqlx::query::QueryScalar;
use sqlx::{PgPool, Postgres};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    group_id: Option<Uuid>,
    title: String,
    description: Option<String>,
    deadline: Option<DateTime<Utc>>,
    auto_delete: Option<bool>,
}

#[post("/task", data = "<data>")]
pub async fn create_task(
    pool: &State<PgPool>,
    auth: AuthUser,
    data: Json<CreateTaskRequest>,
) -> ApiResult {
    let name = data.title.trim();
    if name.len() < 2 || name.len() > 32 {
        return Err(ApiResponse::no_success(
            Status::BadRequest,
            "group name must be between 2 and 32 characters",
        ));
    }

    if let Some(group_id) = data.group_id {
        let is_able = sqlx::query_scalar!("
            SELECT EXISTS(SELECT 1 FROM user_group
                WHERE user_id = $1 AND group_id = $2 AND (permissions = 'owner' OR permissions = 'editor'))",
            auth.id,
            group_id,
        )
            .fetch_one(pool.inner())
            .await
            .map_err(|e| {
                ApiResponse::fail(Status::InternalServerError, "database error", Some(&e))
            })?
            .unwrap_or(false);
        if !is_able {
            return Err(ApiResponse::no_success(
                Status::BadRequest,
                "you need to be owner or editor to create tasks",
            ));
        }
    }
    sqlx::query!(
        "INSERT INTO task (owner_id, group_id, title, description, deadline, auto_delete) VALUES ($1, $2, $3, $4, $5, $6)",
        auth.id,
        data.group_id,
        data.title,
        data.description,
        data.deadline,
        data.auto_delete.unwrap_or(false),
    )
        .execute(pool.inner())
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;
    ApiResponse::ok()
}

#[derive(Deserialize)]
pub struct DeleteTaskRequest {
    id: Uuid,
}
#[delete("/task", data = "<data>")]
pub async fn delete_task(
    pool: &State<PgPool>,
    auth: AuthUser,
    data: Json<DeleteTaskRequest>,
) -> ApiResult {
    let result = sqlx::query!(
        "UPDATE task
         SET deleted_at = NOW()
         WHERE id = $1 AND deleted_at IS NULL
         AND (
             owner_id = $2
             OR group_id IN (
                 SELECT group_id FROM user_group
                 WHERE user_id = $2 AND (permissions = 'owner' OR permissions = 'editor')
             )
         )",
        data.id,
        auth.id
    )
    .execute(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    if result.rows_affected() == 0 {
        return Err(ApiResponse::no_success(
            Status::NotFound,
            "task not found, already deleted, or insufficient permissions",
        ));
    }

    ApiResponse::ok()
}

#[derive(Serialize, Deserialize)]
pub struct AllTaskResponse {
    id: Uuid,
    group_id: Option<Uuid>,
    title: String,
    description: Option<String>,
    deadline: Option<DateTime<Utc>>,
    auto_delete: bool,
    created_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

#[get("/tasks/own")]
pub async fn get_own_tasks(
    pool: &State<PgPool>,
    auth: AuthUser,
) -> ApiResult<Vec<AllTaskResponse>> {
    let tasks = sqlx::query_as!(
        AllTaskResponse,
        r#"SELECT task.id, task.group_id as "group_id?", title, description, deadline, auto_delete, task.created_at, deleted_at FROM task
        WHERE (task.group_id IN (SELECT group_id FROM user_group WHERE user_id = $1)
        AND task.owner_id = $1)
        OR (task.owner_id = $1 AND group_id IS NULL)
        "#,
        auth.id
    )
        .fetch_all(pool.inner())
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    Ok(ApiResponse::success(tasks))
}

#[get("/tasks/my")]
pub async fn get_my_tasks(pool: &State<PgPool>, auth: AuthUser) -> ApiResult<Vec<AllTaskResponse>> {
    let tasks = sqlx::query_as!(
        AllTaskResponse,
        r#"SELECT task.id, task.group_id as "group_id?", title, description, deadline, auto_delete, task.created_at, deleted_at FROM task
        WHERE task.group_id IN (SELECT group_id FROM user_group WHERE user_id = $1)
        OR (task.owner_id = $1 AND group_id IS NULL)
        "#,
        auth.id
    )
        .fetch_all(pool.inner())
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    Ok(ApiResponse::success(tasks))
}

#[derive(Serialize)]
pub struct TaskResponse {
    id: Uuid,
    title: String,
    description: Option<String>,
    deadline: Option<DateTime<Utc>>,
    auto_delete: bool,
    created_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

// EXPERIMENT
// trait AwaitMap<T> {
//     async fn fet_one(&mut self, pool: &PgPool) -> Result<T, (Status, Json<ApiResponse>)>;
// }
//
// impl<T, A> AwaitMap<T> for QueryScalar<'_, Postgres, T, A> {
//     async fn fet_one(&mut self, pool: &PgPool) -> Result<T, (Status, Json<ApiResponse>)> {
//         self
//             .fetch_one(pool)
//             .await
//             .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))
//     }
// }

#[get("/tasks/group?<group_id>")]
pub async fn get_group_tasks(
    pool: &State<PgPool>,
    auth: AuthUser,
    group_id: Uuid,
) -> ApiResult<Vec<TaskResponse>> {
    let tasks = sqlx::query_as!(
        TaskResponse,
        r#"SELECT task.id, title, description, deadline, auto_delete, task.created_at, deleted_at FROM task
        WHERE task.group_id IN (SELECT group_id FROM user_group WHERE user_id = $1)
        AND task.group_id = $2
        "#,
        auth.id,
        group_id
    )
        .fetch_all(pool.inner())
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    Ok(ApiResponse::success(tasks))
    // let is_in_group = sqlx::query_scalar!(
    //     "SELECT EXISTS(SELECT 1 FROM user_group WHERE user_id = $1 AND group_id = $2)",
    //     auth.id,
    //     group_id
    // )
    // .fetch_one(pool.inner())
    // .await
    // .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?
    // .unwrap_or(false);
    //
    // if is_in_group {
    //     let tasks = sqlx::query_as!(
    //         TaskResponse,
    //         "SELECT id, title, description, deadline, auto_delete, created_at, deleted_at FROM task WHERE group_id = $1",
    //         group_id
    //     )
    //         .fetch_all(pool.inner())
    //         .await
    //         .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;
    //     Ok(ApiResponse::success(tasks))
    // } else {
    //     Err(ApiResponse::no_success(
    //         Status::BadRequest,
    //         "group not found",
    //     ))
    // }
}
