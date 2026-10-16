//! Portfolio sample — self-contained EventBus → gacha integration (runnable style).
//! Closer to production `event_tests.rs` without depending on the private AppState harness.

use sqlx::SqlitePool;
use tokio::sync::broadcast::error::RecvError;

use crate::core::event_bus::{AppEvent, EventBus};
use crate::domains::reward::gacha_services::{self, REWARD_STATE_ID};
use crate::tests_samples::common::test_harness::{assert_close, seed_reward_state, setup_pool};

/// Minimal reward listener (same contract as `domains/reward/event_listener.rs`).
fn spawn_gacha_listener(pool: SqlitePool, bus: EventBus) {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::TaskCompleted {
                    task_id,
                    multiplier,
                }) => match gacha_services::run_gacha(&pool, multiplier).await {
                    Ok(Some(drop)) => {
                        tracing::info!(%task_id, amount = drop.amount, "gacha drop");
                    }
                    Ok(None) => {
                        tracing::info!(%task_id, "gacha skipped (budget exhausted)");
                    }
                    Err(err) => tracing::error!(%task_id, error = %err, "gacha failed"),
                },
                Ok(_) => {}
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "reward listener lagged");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

async fn wait_drops(pool: &SqlitePool, min: usize) -> usize {
    for _ in 0..50 {
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM reward_drops WHERE deleted_at IS NULL",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if n as usize >= min {
            return n as usize;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reward_drops WHERE deleted_at IS NULL",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    n as usize
}

#[tokio::test]
async fn task_completed_persists_drop_and_updates_points() {
    let pool = setup_pool().await;
    seed_reward_state(&pool, 0.0, 10_000.0).await;
    let bus = EventBus::default();
    spawn_gacha_listener(pool.clone(), bus.clone());

    bus.publish(AppEvent::TaskCompleted {
        task_id: "task-1".into(),
        multiplier: 1.0,
    });

    assert_eq!(wait_drops(&pool, 1).await, 1);

    let amount: f64 =
        sqlx::query_scalar("SELECT amount FROM reward_drops ORDER BY created_at DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_close(amount, 60.0);

    let points: f64 =
        sqlx::query_scalar("SELECT hidden_points FROM reward_state WHERE id = ?")
            .bind(REWARD_STATE_ID)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_close(points, 60.0);
}

#[tokio::test]
async fn hard_cap_clamps_drop_to_monthly_budget() {
    let pool = setup_pool().await;
    seed_reward_state(&pool, 0.0, 10_000.0).await;
    let bus = EventBus::default();
    spawn_gacha_listener(pool.clone(), bus.clone());

    bus.publish(AppEvent::TaskCompleted {
        task_id: "task-cap".into(),
        multiplier: 1.5,
    });
    assert_eq!(wait_drops(&pool, 1).await, 1);

    let amount: f64 =
        sqlx::query_scalar("SELECT amount FROM reward_drops LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_close(amount, 10_000.0);
}

#[tokio::test]
async fn non_task_events_do_not_insert_drops() {
    let pool = setup_pool().await;
    seed_reward_state(&pool, 0.0, 10_000.0).await;
    let bus = EventBus::default();
    spawn_gacha_listener(pool.clone(), bus.clone());

    bus.publish(AppEvent::AlarmStopped {
        sleep_record_id: "sleep-1".into(),
        target_date: "2026-09-30".into(),
        occurred_at: "2026-09-30T07:00:00Z".into(),
    });
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    assert_eq!(wait_drops(&pool, 0).await, 0);
}

#[tokio::test]
async fn exhausted_budget_skips_additional_drops() {
    let pool = setup_pool().await;
    seed_reward_state(&pool, 0.0, 10_000.0).await;
    let bus = EventBus::default();
    spawn_gacha_listener(pool.clone(), bus.clone());

    bus.publish(AppEvent::TaskCompleted {
        task_id: "fill".into(),
        multiplier: 1.5,
    });
    assert_eq!(wait_drops(&pool, 1).await, 1);

    bus.publish(AppEvent::TaskCompleted {
        task_id: "noop".into(),
        multiplier: 1.0,
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert_eq!(wait_drops(&pool, 1).await, 1);
}
