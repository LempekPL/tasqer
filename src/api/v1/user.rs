use crate::COOKIE_SECURITY;
use crate::password::verify;
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
