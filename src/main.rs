mod api;
mod notifications;
mod password;
mod threads;
mod util;

use crate::util::{ApiResponse, Cors, GuardErrorMessage, connect_db, options_handler};
use rkt::http::Status;
use rkt::serde::json::Json;
use rkt::{Request, catch, launch};

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
        .register(
            "/",
            rkt::catchers![
                unauthorized_catcher,
                forbidden_catcher,
                unprocessable_catcher,
                bad_request_catcher,
                internal_error_catcher
            ],
        )
        .mount("/", rkt::routes![options_handler])
        .mount("/v1", api::v1::routes())
}

#[catch(400)]
pub fn bad_request_catcher(req: &Request) -> Json<ApiResponse> {
    let custom_error = req.local_cache(|| GuardErrorMessage("Bad request".to_string()));
    ApiResponse::no_success(Status::BadRequest, custom_error.0.clone()).1
}

#[catch(401)]
pub fn unauthorized_catcher(req: &Request) -> Json<ApiResponse> {
    let custom_error = req.local_cache(|| GuardErrorMessage("Unauthorized access".to_string()));
    ApiResponse::no_success(Status::Unauthorized, custom_error.0.clone()).1
}

#[catch(403)]
pub fn forbidden_catcher(req: &Request) -> Json<ApiResponse> {
    let custom_error = req.local_cache(|| GuardErrorMessage("Access forbidden".to_string()));
    ApiResponse::no_success(Status::Forbidden, custom_error.0.clone()).1
}

#[catch(422)]
pub fn unprocessable_catcher(req: &Request) -> Json<ApiResponse> {
    let custom_error = req.local_cache(|| GuardErrorMessage("Unprocessable entity".to_string()));
    ApiResponse::no_success(Status::UnprocessableEntity, custom_error.0.clone()).1
}

#[catch(500)]
pub fn internal_error_catcher(req: &Request) -> Json<ApiResponse> {
    let custom_error = req.local_cache(|| GuardErrorMessage("Internal server error".to_string()));
    ApiResponse::no_success(Status::InternalServerError, custom_error.0.clone()).1
}
