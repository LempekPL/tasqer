use rkt::fairing::{Fairing, Info, Kind};
use rkt::http::{Header, Method, Status};
use rkt::serde::json::Json;
use rkt::trace::LevelFilter;
use rkt::{Request, Response, options};
use serde::Serialize;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashSet;
use std::error::Error;
use std::panic::Location;
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

pub struct GuardErrorMessage(pub String);

pub type ApiResult<Success = (), NoSuccess = ()> =
    Result<Json<ApiResponse<Success>>, (Status, Json<ApiResponse<NoSuccess>>)>;

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub error_id: Uuid,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse<T = ()> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl ApiResponse<()> {
    // replace E with ! when it stabilizes
    pub fn ok<E>() -> Result<Json<Self>, E> {
        Ok(Self::success(()))
    }
}

impl<T> ApiResponse<T> {

    pub fn success(data: T) -> Json<Self> {
        Json(Self {
            success: true,
            data: Some(data),
            error: None,
        })
    }

    pub fn no_success(status: Status, message: impl Into<String>) -> (Status, Json<Self>) {
        (
            status,
            Json(Self {
                success: false,
                data: None,
                error: Some(ApiError {
                    error_id: Uuid::now_v7(),
                    message: message.into(),
                }),
            }),
        )
    }

    #[track_caller]
    pub fn fail(status: Status, message: impl ToString, err: Option<&dyn Error>) -> (Status, Json<Self>) {
        let loc = Location::caller();
        let err_id = Uuid::now_v7();

        if let Some(e) = err {
            error!(
                "[{}] {}:{}:{} - {}: {}",
                err_id,
                loc.file(),
                loc.line(),
                loc.column(),
                message.to_string(),
                e
            );
        } else {
            error!(
                "[{}] {}:{}:{} - {}",
                err_id,
                loc.file(),
                loc.line(),
                loc.column(),
                message.to_string()
            );
        }

        (
            status,
            Json(Self {
                success: false,
                data: None,
                error: Some(ApiError {
                    error_id: err_id,
                    message: message.to_string(),
                }),
            }),
        )
    }
}

pub async fn connect_db() -> PgPool {
    info!("Starting database connection...");
    let database_url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to DB");
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let admin_exists = sqlx::query_scalar!("SELECT EXISTS(SELECT 1 FROM users WHERE admin = true)")
        .fetch_one(&pool)
        .await
        .unwrap_or(Some(false))
        .unwrap_or(false);

    if !admin_exists {
        if let (Ok(login), Ok(pass)) = (
            dotenvy::var("INITIAL_ADMIN_LOGIN"),
            dotenvy::var("INITIAL_ADMIN_PASS"),
        ) {
            let hashed = crate::password::hash(pass).await.unwrap();
            sqlx::query!(
            "INSERT INTO users (login, username, password, admin) VALUES ($1, 'Administrator', $2, true)",
            login, hashed
        )
                .execute(&pool)
                .await
                .unwrap();
            info!("Created initial administrator");
        }
    }

    pool
}

// WorkerGuard must be kept alive in "main" for file logging
pub fn setup_logger() -> Option<WorkerGuard> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if cfg!(debug_assertions) {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().with_ansi(true))
            .init();
        info!("Logger started (Console)");
        None
    } else {
        let file_appender = tracing_appender::rolling::daily("logs", "prod.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                fmt::layer()
                    .with_writer(non_blocking)
                    .with_ansi(false)
                    .with_filter(LevelFilter::WARN),
            )
            .init();
        info!("Logger started (File)");
        Some(guard)
    }
}

pub struct Cors {
    allowed_origins: HashSet<String>,
}

impl Cors {
    pub fn new() -> Self {
        let origins_str = dotenvy::var("ALLOWED_ORIGINS").expect("ALLOWED_ORIGINS must be set");

        let allowed_origins: HashSet<String> = origins_str
            .split(',')
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .collect();

        Self { allowed_origins }
    }
}

#[rkt::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "CORS Fairing",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        if let Some(origin) = request.headers().get_one("Origin") {
            if self.allowed_origins.contains(origin) {
                response.set_header(Header::new(
                    "Access-Control-Allow-Origin",
                    origin.to_string(),
                ));
            }
        }
        response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
        response.set_header(Header::new(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization, Accept",
        ));
        response.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "GET, POST, OPTIONS, DELETE, PATCH",
        ));

        if request.method() == Method::Options {
            response.set_status(Status::Ok);
        }
    }
}

#[options("/<_..>")]
pub fn options_handler() -> Status {
    Status::Ok
}
