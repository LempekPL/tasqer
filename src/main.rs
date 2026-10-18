mod api;
mod notifications;
mod password;
mod threads;
mod util;

use crate::util::{Cors, connect_db, options_handler};
use rkt::launch;

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
        .mount("/", rkt::routes![options_handler])
        .mount("/v1", api::v1::routes())
}
