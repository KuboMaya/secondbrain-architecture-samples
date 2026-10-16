//! Portfolio sample — shared in-memory SQLite harness (mirrors `tests/common/mock_db.rs`).
//! Uses `mode=memory&cache=shared` so pool connections see one schema.

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::core::event_bus::EventBus;

/// Shared-cache memory URL so multi-connection pools share tables (production test style).
pub async fn setup_pool() -> SqlitePool {
    let name = Uuid::new_v4().to_string();
    let url = format!("sqlite://file:{name}?mode=memory&cache=shared");
    let pool = SqlitePool::connect(&url).await.expect("test sqlite pool");
    apply_minimal_schema(&pool).await;
    pool
}

pub async fn setup_pool_and_bus() -> (SqlitePool, EventBus) {
    (setup_pool().await, EventBus::default())
}

/// Minimal schema covering task / sleep / reward domains used by portfolio tests.
pub async fn apply_minimal_schema(pool: &SqlitePool) {
    sqlx::query(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;

        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            version INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            title TEXT NOT NULL,
            status TEXT NOT NULL,
            priority INTEGER NOT NULL,
            due_date TEXT,
            parent_id TEXT,
            tags TEXT NOT NULL,
            reward_multiplier REAL NOT NULL
        );

        CREATE TABLE IF NOT EXISTS schedules (
            id TEXT PRIMARY KEY,
            version INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            title TEXT NOT NULL,
            start_time TEXT NOT NULL,
            end_time TEXT NOT NULL,
            is_all_day INTEGER NOT NULL,
            recurrence_rule TEXT,
            location TEXT,
            notes TEXT
        );

        CREATE TABLE IF NOT EXISTS sleep_records (
            id TEXT PRIMARY KEY,
            version INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            target_date TEXT NOT NULL,
            alarm_time TEXT,
            is_alarm_active INTEGER NOT NULL,
            sleep_start TEXT,
            sleep_end TEXT
        );

        CREATE TABLE IF NOT EXISTS reward_state (
            id TEXT PRIMARY KEY,
            version INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            hidden_points REAL NOT NULL,
            monthly_budget REAL NOT NULL
        );

        CREATE TABLE IF NOT EXISTS reward_drops (
            id TEXT PRIMARY KEY,
            version INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            deleted_at TEXT,
            reward_name TEXT NOT NULL,
            amount REAL NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await
    .expect("apply schema");
}

pub const REWARD_STATE_ID: &str = "00000000-0000-4000-8000-000000000001";

pub async fn seed_reward_state(pool: &SqlitePool, hidden: f64, budget: f64) {
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO reward_state (
            id, version, created_at, updated_at, deleted_at, hidden_points, monthly_budget
        ) VALUES (?, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', NULL, ?, ?)
        "#,
    )
    .bind(REWARD_STATE_ID)
    .bind(hidden)
    .bind(budget)
    .execute(pool)
    .await
    .expect("seed reward_state");
}

pub fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}
