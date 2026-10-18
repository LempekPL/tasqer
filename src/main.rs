mod api;
mod notifications;
mod password;
mod threads;
mod util;

use crate::util::{connect_db, options_handler, ApiResponse, Cors, GuardErrorMessage};
use rkt::{catch, launch, Request};
use rkt::http::Status;
use rkt::serde::json::Json;

// easy access to test api with cookies
#[cfg(debug_assertions)]
const COOKIE_SECURITY: bool = false;

// DO NOT TOUCH
#[cfg(not(debug_assertions))]
const COOKIE_SECURITY: bool = true;

#[launch]
async fn rocket() -> _ {
    let _log_guard = util::setup_logger();
    let pool = connect_db().await;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    threads::spawn_cleanup_worker(pool.clone());
    threads::spawn_deadline_worker(pool.clone(), tx.clone());
    notifications::spawn_notification_sender_worker(rx);
    rkt::build()
        .manage(pool)
        .attach(Cors::new())
        .register("/", rkt::catchers![unauthorized_catcher])
        .mount("/", rkt::routes![options_handler])
        .mount("/v1", api::v1::routes())
}

#[catch(401)]
pub fn unauthorized_catcher(req: &Request) -> Json<ApiResponse> {
    let custom_error = req.local_cache(|| GuardErrorMessage("Unauthorized access".to_string()));
    ApiResponse::no_success(Status::Unauthorized, custom_error.0.clone()).1
}
