//! Portfolio sample — TaskCompleted → gacha → RewardDropped (background).
//! Completes the 0ms task-complete story: HTTP returns before this runs.
//! Source: domains/reward/event_listener.rs

use sqlx::SqlitePool;
use tokio::sync::broadcast::error::RecvError;

use super::services;
use crate::core::event_bus::{AppEvent, EventBus};

/// Subscribe to TaskCompleted and run gacha in the background. Never panics on lag.
pub fn spawn(pool: SqlitePool, bus: EventBus) {
    let mut rx = bus.subscribe();
    let bus_out = bus.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::TaskCompleted {
                    task_id,
                    multiplier,
                }) => match services::run_gacha(&pool, multiplier).await {
                    Ok(Some(drop)) => {
                        tracing::info!(
                            task_id = %task_id,
                            multiplier,
                            amount = drop.amount,
                            "gacha drop recorded"
                        );
                        bus_out.publish(AppEvent::RewardDropped {
                            task_id,
                            drop_id: drop.id,
                            reward_name: drop.reward_name,
                            amount: drop.amount,
                        });
                    }
                    Ok(None) => {
                        tracing::info!(
                            task_id = %task_id,
                            multiplier,
                            "gacha skipped (budget / hard-cap exhausted)"
                        );
                    }
                    Err(err) => {
                        tracing::error!(task_id = %task_id, error = %err, "gacha failed");
                    }
                },
                Ok(_) => {}
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "reward listener lagged; continuing");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}
