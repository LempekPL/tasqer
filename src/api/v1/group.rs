use crate::api::PermissionRole;
use crate::api::v1::user::AuthUser;
use crate::util::{ApiResponse, ApiResult};
use rkt::http::Status;
use rkt::serde::json::Json;
use rkt::{State, delete, get, post};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize)]
pub struct GroupResponse {
    pub id: Uuid,
    pub name: String,
}

#[get("/groups")]
pub async fn get_groups(auth: AuthUser, pool: &State<PgPool>) -> ApiResult<Vec<GroupResponse>> {
    let groups = sqlx::query_as!(
        GroupResponse,
        "SELECT id, name FROM groups WHERE public = true ORDER BY id DESC"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;
    Ok(ApiResponse::success(groups))
}

#[derive(Deserialize)]
pub struct CreateGroupRequest {
    pub name: String,
    pub public: bool,
}

#[derive(Serialize)]
pub struct CreateGroupResponse {
    pub id: Uuid,
    pub name: String,
    pub public: bool,
    pub role: String,
}

#[post("/group", data = "<data>")]
pub async fn create_group(
    auth: AuthUser,
    pool: &State<PgPool>,
    data: Json<CreateGroupRequest>,
) -> ApiResult<CreateGroupResponse> {
    let name = data.name.trim();

    if name.len() < 2 || name.len() > 50 {
        return Err(ApiResponse::no_success(
            Status::BadRequest,
            "group name must be between 2 and 50 characters",
        ));
    }

    let mut tx =
        pool.inner().begin().await.map_err(|e| {
            ApiResponse::fail(Status::InternalServerError, "database error", Some(&e))
        })?;

    let group_id = sqlx::query_scalar!(
        "INSERT INTO groups (name, public) VALUES ($1, $2) RETURNING id",
        name,
        data.public
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    sqlx::query!(
        "INSERT INTO user_group (user_id, group_id, permissions) VALUES ($1, $2, 'owner')",
        auth.id,
        group_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    tx.commit()
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    Ok(ApiResponse::success(CreateGroupResponse {
        id: group_id,
        name: name.to_string(),
        public: data.public,
        role: "owner".to_string(),
    }))
}

#[derive(Deserialize)]
pub struct DeleteGroupRequest {
    pub id: Uuid,
}

#[delete("/group", data = "<data>")]
pub async fn delete_group(
    auth: AuthUser,
    pool: &State<PgPool>,
    data: Json<DeleteGroupRequest>,
) -> ApiResult {
    let user_role = sqlx::query_scalar!(
        "SELECT permissions FROM user_group WHERE group_id = $1 AND user_id = $2",
        data.id,
        auth.id
    )
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    match user_role {
        Some(role) if role == "owner" => {
            sqlx::query!("DELETE FROM groups WHERE id = $1", data.id)
                .execute(pool.inner())
                .await
                .map_err(|e| {
                    ApiResponse::fail(Status::InternalServerError, "database error", Some(&e))
                })?;
            Ok(ApiResponse::success(()))
        }
        Some(_) => Err(ApiResponse::no_success(
            Status::Forbidden,
            "only the group owner can delete it",
        )),
        None => Err(ApiResponse::no_success(
            Status::NotFound,
            "group not found or access denied",
        )),
    }
}

#[derive(Deserialize)]
pub struct JoinGroupRequest {
    pub id: Uuid,
}

#[derive(Serialize)]
pub struct JoinGroupResponse {
    pub id: Uuid,
    pub role: String,
}

#[post("/group/join", data = "<data>")]
pub async fn join_group(
    auth: AuthUser,
    pool: &State<PgPool>,
    data: Json<JoinGroupRequest>,
) -> ApiResult<JoinGroupResponse> {
    let group_is_public = sqlx::query_scalar!("SELECT public FROM groups WHERE id = $1", data.id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    match group_is_public {
        Some(true) => {
            sqlx::query!(
                "INSERT INTO user_group (user_id, group_id, permissions) VALUES ($1, $2, 'viewer')",
                auth.id,
                data.id
            )
            .execute(pool.inner())
            .await
            .map_err(|e| {
                ApiResponse::fail(Status::InternalServerError, "database error", Some(&e))
            })?;

            Ok(ApiResponse::success(JoinGroupResponse {
                id: data.id,
                role: "viewer".to_string(),
            }))
        }
        Some(false) => Err(ApiResponse::no_success(
            Status::Forbidden,
            "this group is private",
        )),
        None => Err(ApiResponse::no_success(Status::NotFound, "group not found")),
    }
}

#[derive(Deserialize)]
pub struct AddGroupRequest {
    pub user_id: Uuid,
    pub group_id: Uuid,
}

#[post("/group/add", data = "<data>")]
pub async fn add_to_group(
    auth: AuthUser,
    pool: &State<PgPool>,
    data: Json<AddGroupRequest>,
) -> ApiResult {
    let group_meta = sqlx::query!(
        r#"
        SELECT g.public, ug.permissions as "permissions: PermissionRole"
        FROM groups g
        LEFT JOIN user_group ug ON g.id = ug.group_id AND ug.user_id = $2
        WHERE g.id = $1
        "#,
        data.group_id,
        auth.id
    )
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    let meta = match group_meta {
        Some(m) => m,
        None => return Err(ApiResponse::no_success(Status::NotFound, "group not found")),
    };

    let auth_role = match meta.permissions {
        Some(role) => role,
        None => return Err(ApiResponse::no_success(Status::Forbidden, "access denied")),
    };

    let can_add = match (meta.public, auth_role) {
        (false, PermissionRole::Owner) => true,
        (true, PermissionRole::Owner | PermissionRole::Editor) => true,
        _ => false,
    };

    if !can_add {
        return Err(ApiResponse::no_success(
            Status::Forbidden,
            "you don't have permission to add users to this group",
        ));
    }

    let is_already_member = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM user_group WHERE user_id = $1 AND group_id = $2)",
        data.user_id,
        data.group_id
    )
    .fetch_one(pool.inner())
    .await
    .unwrap_or(Some(false))
    .unwrap_or(false);

    if is_already_member {
        return Err(ApiResponse::no_success(
            Status::Conflict,
            "user is already in the group",
        ));
    }

    sqlx::query!(
        "INSERT INTO user_group (user_id, group_id, permissions) VALUES ($1, $2, 'viewer')",
        data.user_id,
        data.group_id
    )
    .execute(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    Ok(ApiResponse::success(()))
}
