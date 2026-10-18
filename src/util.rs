use rkt::fairing::{Fairing, Info, Kind};
use rkt::http::{Header, Method, Status};
use rkt::{Request, Response, options};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashSet;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub async fn connect_db() -> PgPool {
    info!("Starting database connection...");
    let database_url = dotenvy::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to DB");
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
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
            .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
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
