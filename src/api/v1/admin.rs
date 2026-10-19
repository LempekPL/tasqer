use crate::api::PermissionRole;
use crate::api::v1::user::AuthUser;
use crate::util::{ApiResponse, ApiResult, GuardErrorMessage};
use chrono::{DateTime, Utc};
use rand::RngExt;
use rkt::http::Status;
use rkt::request::{FromRequest, Outcome};
use rkt::serde::Deserialize;
use rkt::serde::json::Json;
use rkt::{Request, State, delete, get, post};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Serialize)]
pub struct AdminUserView {
    pub id: Uuid,
    pub login: String,
    pub username: String,
    pub admin: bool,
    pub private: bool,
    pub created_at: DateTime<Utc>,
}

#[get("/admin/users")]
pub async fn get_users(
    _auth: AuthAdminUser,
    pool: &State<PgPool>,
) -> ApiResult<Vec<AdminUserView>> {
    let users = sqlx::query_as!(
        AdminUserView,
        "SELECT id, login, username, admin, private, created_at FROM users ORDER BY created_at DESC"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;
    Ok(ApiResponse::success(users))
}

#[derive(Serialize)]
pub struct GroupUserView {
    pub id: Uuid,
    pub username: String,
    pub permissions: PermissionRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct AdminGroupView {
    pub id: Uuid,
    pub name: String,
    pub public: bool,
    pub created_at: DateTime<Utc>,
    pub users: Vec<GroupUserView>,
}

#[get("/admin/groups")]
pub async fn get_groups(
    _auth: AuthAdminUser,
    pool: &State<PgPool>,
) -> ApiResult<Vec<AdminGroupView>> {
    let records = sqlx::query!(
        r#"
        SELECT
            g.id as group_id,
            g.name as group_name,
            g.public,
            g.created_at as group_created_at,
            u.id as "user_id?",
            u.username as "username?",
            ug.permissions as "permissions?: PermissionRole",
            ug.created_at as "joined_at?"
        FROM groups g
        LEFT JOIN user_group ug ON g.id = ug.group_id
        LEFT JOIN users u ON ug.user_id = u.id
        ORDER BY g.created_at DESC, g.id DESC, ug.created_at
        "#
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    let mut groups: Vec<AdminGroupView> = Vec::new();

    for row in records {
        let user_opt = match (row.user_id, row.username, row.permissions, row.joined_at) {
            (Some(id), Some(username), Some(permissions), Some(joined_at)) => Some(GroupUserView {
                id,
                username,
                permissions,
                joined_at,
            }),
            _ => None,
        };

        let is_same_group = groups.last().map_or(false, |g| g.id == row.group_id);

        if is_same_group {
            if let Some(user) = user_opt {
                groups.last_mut().unwrap().users.push(user);
            }
        } else {
            let users = user_opt.map_or_else(Vec::new, |u| vec![u]);

            groups.push(AdminGroupView {
                id: row.group_id,
                name: row.group_name,
                public: row.public,
                created_at: row.group_created_at,
                users,
            });
        }
    }

    Ok(ApiResponse::success(groups))
}

#[derive(Deserialize)]
pub struct DeleteUserRequest {
    pub id: Uuid,
}

#[delete("/admin/user", data = "<data>")]
pub async fn delete_user(
    auth: AuthAdminUser,
    pool: &State<PgPool>,
    data: Json<DeleteUserRequest>,
) -> ApiResult {
    if data.id == auth.0.id {
        return Err(ApiResponse::no_success(
            Status::BadRequest,
            "you cannot delete yourself",
        ));
    }
    let result = sqlx::query!("DELETE FROM users WHERE id = $1", data.id)
        .execute(pool.inner())
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;
    if result.rows_affected() == 0 {
        return Err(ApiResponse::no_success(Status::NotFound, "user not found"));
    }
    Ok(ApiResponse::success(()))
}

#[derive(Serialize)]
pub struct InviteResponse {
    pub code: String,
}

fn generate_invite_code() -> String {
    let mut rng = rand::rng();
    let random_part: String = (&mut rng)
        .sample_iter(&rand::distr::Alphanumeric)
        .take(4)
        .map(|c| char::from(c).to_ascii_uppercase())
        .collect();
    let date_num: u32 = Utc::now().format("%Y%m%d").to_string().parse().unwrap_or(0);
    format!("TS-{}-{:X}", random_part.to_uppercase(), date_num)
}

#[post("/admin/invite")]
pub async fn create_invite(auth: AuthAdminUser, pool: &State<PgPool>) -> ApiResult<InviteResponse> {
    let invite_code = generate_invite_code();
    sqlx::query!(
        "INSERT INTO invite_codes (code, created_by) VALUES ($1, $2)",
        invite_code,
        auth.0.id
    )
    .execute(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;
    Ok(ApiResponse::success(InviteResponse { code: invite_code }))
}

#[derive(Clone)]
pub struct AuthAdminUser(pub AuthUser);

#[rkt::async_trait]
impl<'r> FromRequest<'r> for AuthAdminUser {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let user_outcome = AuthUser::from_request(request).await;

        match user_outcome {
            Outcome::Success(user) => {
                if user.admin {
                    Outcome::Success(AuthAdminUser(user))
                } else {
                    request
                        .local_cache(|| GuardErrorMessage("admin privileges required".to_string()));
                    Outcome::Error((Status::Forbidden, ()))
                }
            }
            Outcome::Error(e) => Outcome::Error(e),
            Outcome::Forward(f) => Outcome::Forward(f),
        }
    }
}
