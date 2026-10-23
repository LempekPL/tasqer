use crate::COOKIE_SECURITY;
use crate::password::{hash, verify};
use crate::util::{ApiResponse, ApiResult, GuardErrorMessage};
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use rand::Rng;
use rkt::http::{Cookie, CookieJar, SameSite, Status};
use rkt::request::{FromRequest, Outcome};
use rkt::serde::json::Json;
use rkt::{Request, State, delete, get, post};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use crate::api::PermissionRole;
// TODO: make "deleted" user be "deleted" and not deleted 👍

fn generate_session_token() -> String {
    let mut key = [0u8; 32];
    rand::rngs::ThreadRng::default().fill_bytes(&mut key);
    BASE64_URL_SAFE_NO_PAD.encode(key)
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub login: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub id: Uuid,
    pub username: String,
    pub admin: bool,
}

#[post("/user/login", data = "<data>")]
pub async fn login(
    pool: &State<PgPool>,
    cookies: &CookieJar<'_>,
    data: Json<LoginRequest>,
) -> ApiResult<LoginResponse> {
    let user_record = sqlx::query!(
        "SELECT id, login, password, username, admin FROM users WHERE login = $1",
        data.login
    )
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    let user = match user_record {
        Some(u) => u,
        None => {
            return Err(ApiResponse::no_success(
                Status::Unauthorized,
                "wrong login or password",
            ));
        }
    };

    let is_valid = verify(data.password.clone(), user.password)
        .await
        .map_err(|e| {
            ApiResponse::fail(
                Status::InternalServerError,
                "internal server error",
                e.source(),
            )
        })?;

    if !is_valid {
        return Err(ApiResponse::no_success(
            Status::Unauthorized,
            "wrong login or password",
        ));
    }

    let token = generate_session_token();

    sqlx::query!(
        r#"
        INSERT INTO sessions (user_id, token, expires_at)
        VALUES ($1, $2, NOW() + INTERVAL '30 days')
        "#,
        user.id,
        token
    )
    .execute(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    cookies.add(
        Cookie::build(("session_token", token.clone()))
            .http_only(true)
            .secure(COOKIE_SECURITY)
            .same_site(SameSite::Lax)
            .build(),
    );

    Ok(ApiResponse::success(LoginResponse {
        token,
        id: user.id,
        username: user.username,
        admin: user.admin,
    }))
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub login: String,
    pub username: String,
    pub password: String,
    pub invite: String,
}

#[post("/user/register", data = "<data>")]
pub async fn register(
    pool: &State<PgPool>,
    cookies: &CookieJar<'_>,
    data: Json<RegisterRequest>,
) -> ApiResult<LoginResponse> {
    let login = data.login.trim();
    if login.len() < 3 || login.len() > 32 {
        return Err(ApiResponse::no_success(
            Status::BadRequest,
            "login must be between 3 and 32 characters",
        ));
    }
    if !login
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return Err(ApiResponse::no_success(
            Status::BadRequest,
            "login can only contain lowercase letters and numbers",
        ));
    }

    let username = data.username.trim();
    if username.len() < 2 || username.len() > 50 {
        return Err(ApiResponse::no_success(
            Status::BadRequest,
            "username must be between 2 and 50 characters",
        ));
    }

    let password = &data.password;
    if password.len() < 8 || password.len() > 64 {
        return Err(ApiResponse::no_success(
            Status::BadRequest,
            "password must be between 8 and 64 characters",
        ));
    }
    let has_alphabetic = password.chars().any(|c| c.is_ascii_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !(has_alphabetic && has_digit) {
        return Err(ApiResponse::no_success(
            Status::BadRequest,
            "password must contain at least one letter and one number",
        ));
    }

    let mut tx =
        pool.inner().begin().await.map_err(|e| {
            ApiResponse::fail(Status::InternalServerError, "database error", Some(&e))
        })?;

    let invite_record = sqlx::query!(
        "SELECT id FROM invite_codes WHERE code = $1 AND used_by IS NULL FOR UPDATE",
        data.invite
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    let invite_id = match invite_record {
        Some(record) => record.id,
        None => {
            return Err(ApiResponse::no_success(
                Status::BadRequest,
                "invalid or already used invite code",
            ));
        }
    };

    let user_exists =
        sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM users WHERE login = $1)", login)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(Some(false))
            .unwrap_or(false);

    if user_exists {
        return Err(ApiResponse::no_success(
            Status::Conflict,
            "login is already taken",
        ));
    }

    let hashed_password = hash(password.clone()).await.map_err(|e| {
        ApiResponse::fail(
            Status::InternalServerError,
            "internal server error",
            e.source(),
        )
    })?;

    let new_user_id = sqlx::query_scalar!(
        r#"
        INSERT INTO users (login, username, password, admin)
        VALUES ($1, $2, $3, false)
        RETURNING id
        "#,
        login,
        username,
        hashed_password
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    sqlx::query!(
        "UPDATE invite_codes SET used_by = $1 WHERE id = $2",
        new_user_id,
        invite_id
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    let token = generate_session_token();

    sqlx::query!(
        r#"
        INSERT INTO sessions (user_id, token, expires_at)
        VALUES ($1, $2, NOW() + INTERVAL '30 days')
        "#,
        new_user_id,
        token
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    tx.commit()
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    cookies.add(
        Cookie::build(("session_token", token.clone()))
            .http_only(true)
            .secure(COOKIE_SECURITY)
            .same_site(SameSite::Lax)
            .build(),
    );

    Ok(ApiResponse::success(LoginResponse {
        token,
        id: new_user_id,
        username: username.to_string(),
        admin: false,
    }))
}

#[derive(Serialize)]
struct UserGroupInfo {
    group_id: Uuid,
    name: String,
    permissions: String,
}

#[derive(Serialize)]
pub struct User {
    id: Uuid,
    username: String,
    admin: bool,
    created_at: DateTime<Utc>,
    groups: Vec<UserGroupInfo>,
}

#[get("/user/me")]
pub async fn get_self(user: AuthUser, pool: &State<PgPool>) -> ApiResult<User> {
    let user_data = sqlx::query!(
        "SELECT id, username, admin, created_at FROM users WHERE id = $1",
        user.id
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    let groups = sqlx::query_as!(
        UserGroupInfo,
        r#"
        SELECT g.id as group_id, g.name, ug.permissions
        FROM groups g
        JOIN user_group ug ON g.id = ug.group_id
        WHERE ug.user_id = $1
        "#,
        user.id
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;

    Ok(ApiResponse::success(User {
        id: user_data.id,
        username: user_data.username,
        admin: user_data.admin,
        created_at: user_data.created_at,
        groups,
    }))
}

#[delete("/user/logout")]
pub async fn logout(user: AuthUser, pool: &State<PgPool>, cookies: &CookieJar<'_>) -> ApiResult {
    cookies.remove(Cookie::from("session_token"));
    sqlx::query!("DELETE FROM sessions WHERE token = $1", user.token)
        .execute(pool.inner())
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;
    Ok(ApiResponse::success(()))
}

#[derive(Serialize)]
pub struct UserGroupResponse {
    pub id: Uuid,
    pub name: String,
    pub public: bool,
    pub permissions: PermissionRole,
}

#[get("/user/groups")]
pub async fn get_groups(auth: AuthUser, pool: &State<PgPool>) -> ApiResult<Vec<UserGroupResponse>> {
    let groups = sqlx::query_as!(
        UserGroupResponse,
        r#"SELECT g.id, g.name, g.public, ug.permissions as "permissions: PermissionRole" FROM groups g INNER JOIN user_group ug ON ug.group_id = g.id WHERE ug.user_id = $1"#,
        auth.id
    )
        .fetch_all(pool.inner())
        .await
        .map_err(|e| ApiResponse::fail(Status::InternalServerError, "database error", Some(&e)))?;
    Ok(ApiResponse::success(groups))
}

#[derive(Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub token: String,
    pub admin: bool,
}

#[rkt::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let user_result = request
            .local_cache_async(async {
                let pool = request.guard::<&State<PgPool>>().await.unwrap();

                let token = request
                    .headers()
                    .get_one("Authorization")
                    .map(|t| t.replace("Bearer ", ""))
                    .or_else(|| {
                        request
                            .cookies()
                            .get("session_token")
                            .map(|c| c.value().to_string())
                    });

                let token = match token {
                    Some(t) => t,
                    None => {
                        request.local_cache(|| GuardErrorMessage("No authentication token provided".to_string()));
                        return None;
                    }
                };

                let user = sqlx::query!(r#"
                    SELECT
                        u.id, u.admin,
                        (s.expires_at < NOW() + INTERVAL '14 days') as needs_refresh
                    FROM users u
                    JOIN sessions s ON u.id = s.user_id
                    WHERE s.token = $1 AND s.expires_at > NOW()
                "#,
                    token
                )
                    .fetch_optional(pool.inner())
                    .await
                    .ok()??;

                if user.needs_refresh.unwrap_or(false) {
                    let pool_clone = pool.inner().clone();
                    let token_clone = token.clone();

                    // spawning single one off task to not delay the request
                    tokio::spawn(async move {
                        let _ = sqlx::query!(
                            "UPDATE sessions SET expires_at = NOW() + INTERVAL '30 days' WHERE token = $1",
                            token_clone
                        )
                            .execute(&pool_clone)
                            .await;
                    });
                }

                Some(AuthUser {
                    id: user.id,
                    token,
                    admin: user.admin,
                })
            })
            .await;

        match user_result {
            Some(user) => Outcome::Success(user.clone()),
            None => {
                request.local_cache(|| GuardErrorMessage("You are not authenticated".to_string()));
                Outcome::Error((Status::Unauthorized, ()))
            }
        }
    }
}
