//! Portfolio sample — EventBus integration: TaskCompleted → gacha drop.
//! Source: excerpt from `src-tauri/tests/reward/event_tests.rs`
//! (`common::mock_db` harness omitted — see private repo tests/).

use second_brain::core::event_bus::AppEvent;
use second_brain::domains::reward::services;

#[tokio::test]
async fn task_completed_event_creates_drop() {
    let state = common::mock_db::setup_state().await;
    second_brain::domains::reward::event_listener::spawn(
        state.pool.clone(),
        state.event_bus.clone(),
    );

    state.event_bus.publish(AppEvent::TaskCompleted {
        task_id: "task-1".into(),
        multiplier: 1.0,
    });

    let mut found = None;
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        let drops = services::list_drops(&state.pool).await.unwrap();
        if let Some(drop) = drops.first() {
            found = Some(drop.clone());
            break;
        }
    }
    let drop = found.expect("gacha should persist a drop");
    assert_eq!(drop.reward_name, "gacha");
}

#[tokio::test]
async fn ignored_events_do_not_create_drops() {
    let state = common::mock_db::setup_state().await;
    second_brain::domains::reward::event_listener::spawn(
        state.pool.clone(),
        state.event_bus.clone(),
    );
    state.event_bus.publish(AppEvent::AlarmStopped {
        sleep_record_id: "sleep-1".into(),
        target_date: "2026-09-30".into(),
        occurred_at: "2026-09-30T07:00:00Z".into(),
    });
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let drops = services::list_drops(&state.pool).await.unwrap();
    assert!(drops.is_empty());
}
