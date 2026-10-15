//! Portfolio sample — SQLite pool with WAL + foreign_keys + migrations.
//! Source: src-tauri/src/core/db.rs

use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;

use super::error::{AppError, AppResult};

const REWARD_STATE_ID: &str = "00000000-0000-4000-8000-000000000001";

/// Default local DB URL used when `DATABASE_URL` is unset.
pub const DEFAULT_DATABASE_URL: &str = "sqlite://secondbrain.db";

/// Create a SQLite pool with WAL / foreign_keys, then apply sqlx migrations.
pub async fn init_pool(database_url: &str) -> AppResult<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)
        .map_err(|err| AppError::Internal(format!("invalid DATABASE_URL: {err}")))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let is_memory = database_url.contains(":memory:")
        || database_url.contains("mode=memory")
        || database_url.ends_with("sqlite::memory:");
    let max_connections = if is_memory { 1 } else { 8 };

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await?;

    // Required by architecture: enforce WAL on the live connection.
    sqlx::query("PRAGMA journal_mode=WAL;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys=ON;")
        .execute(&pool)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

pub fn reward_state_id() -> &'static str {
    REWARD_STATE_ID
}

/// Tables that participate in 7-day tombstone garbage collection.
pub fn gc_tables() -> &'static [&'static str] {
    &[
        "tasks",
        "schedules",
        "dictionary_indexes",
        "dictionary",
        "inventory_categories",
        "inventory",
        "sleep_records",
        "weather_forecasts",
        "weather_radar_snapshots",
        "reward_drops",
        "ai_logs",
        "device_identities",
        "device_pairing_requests",
    ]
}
