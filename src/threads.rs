use crate::notifications::Notification;
use sqlx::PgPool;
use std::fs;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tracing::{error, info};

pub fn spawn_deadline_worker(pool: PgPool, notifier: mpsc::Sender<Notification>) {
    tokio::spawn(async move {
        info!("Deadline worker started.");
        let mut interval = tokio::time::interval(Duration::from_secs(60));

        loop {
            interval.tick().await;
            let mut tx = match pool.begin().await {
                Ok(transaction) => transaction,
                Err(e) => {
                    error!("Failed to begin transaction for deadlines: {}", e);
                    continue;
                }
            };
            let auto_deleted_result = sqlx::query!(
                r#"
                UPDATE task
                SET deleted_at = NOW(), deadline_notified = true
                WHERE deadline <= NOW()
                  AND deleted_at IS NULL
                  AND auto_delete = true
                  AND deadline_notified = false
                RETURNING id, title, owner_id
                "#
            )
            .fetch_all(&mut *tx)
            .await;

            if let Ok(tasks) = auto_deleted_result {
                for task in tasks {
                    let msg = Notification::TaskDeadlineReached {
                        task_id: task.id,
                        title: task.title,
                        owner_id: task.owner_id,
                        soft_deleted: true,
                    };
                    let _ = notifier.try_send(msg);
                }
            } else if let Err(e) = auto_deleted_result {
                error!("Error processing auto-delete deadlines: {}", e);
            }

            let notified_result = sqlx::query!(
                r#"
                UPDATE task
                SET deadline_notified = true
                WHERE deadline <= NOW()
                  AND deleted_at IS NULL
                  AND auto_delete = false
                  AND deadline_notified = false
                RETURNING id, title, owner_id
                "#
            )
            .fetch_all(&mut *tx)
            .await;

            if let Ok(tasks) = &notified_result {
                for task in tasks {
                    let msg = Notification::TaskDeadlineReached {
                        task_id: task.id,
                        title: task.title.clone(),
                        owner_id: task.owner_id,
                        soft_deleted: false,
                    };
                    let _ = notifier.try_send(msg);
                }
            } else if let Err(e) = notified_result {
                error!("Error processing notification deadlines: {}", e);
            }
            if let Err(e) = tx.commit().await {
                error!("Failed to commit deadline transaction: {}", e);
            }
        }
    });
}

pub fn spawn_cleanup_worker(pool: PgPool) {
    tokio::spawn(async move {
        info!("Cleanup worker started.");
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 60 * 24));

        loop {
            interval.tick().await;
            // clean up sessions
            let result = sqlx::query!("DELETE FROM sessions WHERE expires_at < NOW()")
                .execute(&pool)
                .await;
            match result {
                Ok(res) => {
                    if res.rows_affected() > 0 {
                        info!("Cleaned up {} expired sessions.", res.rows_affected());
                    }
                }
                Err(e) => error!("Failed to clean up expired sessions: {}", e),
            }
            // clean up old logs >30 days
            if let Ok(entries) = fs::read_dir("logs") {
                let thirty_days_ago = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 30);
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata()
                        && let Ok(modified) = metadata.modified()
                        && modified < thirty_days_ago
                    {
                        if let Err(e) = fs::remove_file(entry.path()) {
                            error!("Failed to remove old log file {:?}: {}", entry.path(), e);
                        } else {
                            info!("Removed old log file: {:?}", entry.path());
                        }
                    }
                }
            }
        }
    });
}
