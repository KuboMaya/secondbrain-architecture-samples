//! Portfolio sample — EventBus reward scenarios (production naming).
//! Full self-contained implementation: see `reward_bus_integration.rs`.
//!
//! Covered scenarios (same as private `tests/reward/event_tests.rs`):
//! - TaskCompleted → gacha drop + points
//! - hard-capped multiplier clamps to monthly_budget
//! - unrelated events (AlarmStopped) create no drops
//! - exhausted budget skips additional drops
//!
//! Run target in the private monorepo:
//! `cargo test -p second_brain --test reward -- event`

#[allow(dead_code)]
fn see_reward_bus_integration() {
    // Intentionally empty: analyzers should open reward_bus_integration.rs
    // which contains the async tokio::test bodies + shared-cache harness.
}
