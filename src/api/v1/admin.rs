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
    pub created_at: DateTime<Utc>,
}

#[get("/admin/users")]
pub async fn get_users(
    _auth: AuthAdminUser,
    pool: &State<PgPool>,
) -> ApiResult<Vec<AdminUserView>> {
    let users = sqlx::query_as!(
        AdminUserView,
        "SELECT id, login, username, admin, created_at FROM users ORDER BY created_at DESC"
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;
    Ok(ApiResponse::success(users))
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
