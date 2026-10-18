use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

#[derive(Debug)]
pub enum Notification {
    TaskDeadlineReached {
        task_id: Uuid,
        title: String,
        owner_id: Uuid,
        soft_deleted: bool,
    },
}

pub fn spawn_notification_sender_worker(mut rx: mpsc::Receiver<Notification>) {
    tokio::spawn(async move {
        info!("Notification sender worker started.");

        while let Some(notification) = rx.recv().await {
            match notification {
                Notification::TaskDeadlineReached { task_id, title, owner_id, soft_deleted } => {
                    info!(
                        "Sending notification for task '{}' (Owner: {}). Moved to trash: {}",
                        title, owner_id, soft_deleted
                    );
                    // TODO: actually send notifications
                }
            }
        }
    });
}