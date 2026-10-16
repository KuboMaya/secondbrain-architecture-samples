//! Portfolio sample — run_gacha / spend / undo (sqlx + budget math).
//! Source: `domains/reward/services.rs` (IDs are synthetic UUIDs in tests).

use sqlx::SqlitePool;
use uuid::Uuid;

use super::gacha_math::{
    apply_soft_cap, evaluate_gacha, is_allowed_spend, SPEND_AMOUNTS,
};
use crate::core::error::{AppError, AppResult};

pub const SPEND_REWARD_NAME: &str = "spend";
pub const REWARD_STATE_ID: &str = "00000000-0000-4000-8000-000000000001";

#[derive(Debug, Clone, sqlx::FromRow, PartialEq)]
pub struct RewardState {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub hidden_points: f64,
    pub monthly_budget: f64,
}

#[derive(Debug, Clone, sqlx::FromRow, PartialEq)]
pub struct RewardDrop {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub reward_name: String,
    pub amount: f64,
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub async fn get_state(pool: &SqlitePool) -> AppResult<RewardState> {
    sqlx::query_as::<_, RewardState>("SELECT * FROM reward_state WHERE id = ?")
        .bind(REWARD_STATE_ID)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("reward_state".into()))
}

async fn set_hidden_points(pool: &SqlitePool, state: &mut RewardState, points: f64) -> AppResult<()> {
    state.hidden_points = points.max(0.0);
    state.version += 1;
    state.updated_at = now_rfc3339();
    sqlx::query(
        "UPDATE reward_state SET hidden_points = ?, version = ?, updated_at = ? WHERE id = ?",
    )
    .bind(state.hidden_points)
    .bind(state.version)
    .bind(&state.updated_at)
    .bind(&state.id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn run_gacha(pool: &SqlitePool, reward_multiplier: f64) -> AppResult<Option<RewardDrop>> {
    let mut state = get_state(pool).await?;
    let (amount, _ratio, _hard) =
        evaluate_gacha(reward_multiplier, state.hidden_points, state.monthly_budget);
    if amount <= 0.0 {
        return Ok(None);
    }

    let now = now_rfc3339();
    let drop = RewardDrop {
        id: Uuid::new_v4().to_string(),
        version: 1,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
        reward_name: "gacha".into(),
        amount,
    };

    sqlx::query(
        r#"
        INSERT INTO reward_drops (
            id, version, created_at, updated_at, deleted_at, reward_name, amount
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&drop.id)
    .bind(drop.version)
    .bind(&drop.created_at)
    .bind(&drop.updated_at)
    .bind(&drop.deleted_at)
    .bind(&drop.reward_name)
    .bind(drop.amount)
    .execute(pool)
    .await?;

    let next = state.hidden_points + amount;
    set_hidden_points(pool, &mut state, next).await?;
    Ok(Some(drop))
}

pub async fn spend_points(pool: &SqlitePool, amount: f64) -> AppResult<(RewardState, RewardDrop)> {
    if !is_allowed_spend(amount) {
        return Err(AppError::Validation(format!(
            "amount must be one of {:?}",
            SPEND_AMOUNTS
        )));
    }
    let mut state = get_state(pool).await?;
    if state.hidden_points + f64::EPSILON < amount {
        return Err(AppError::Validation("insufficient points".into()));
    }

    let now = now_rfc3339();
    let drop = RewardDrop {
        id: Uuid::new_v4().to_string(),
        version: 1,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
        reward_name: SPEND_REWARD_NAME.into(),
        amount,
    };
    sqlx::query(
        r#"
        INSERT INTO reward_drops (
            id, version, created_at, updated_at, deleted_at, reward_name, amount
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&drop.id)
    .bind(drop.version)
    .bind(&drop.created_at)
    .bind(&drop.updated_at)
    .bind(&drop.deleted_at)
    .bind(&drop.reward_name)
    .bind(drop.amount)
    .execute(pool)
    .await?;

    set_hidden_points(pool, &mut state, state.hidden_points - amount).await?;
    Ok((state, drop))
}

pub async fn apply_daily_soft_cap(pool: &SqlitePool) -> AppResult<RewardState> {
    let mut state = get_state(pool).await?;
    let clamped = apply_soft_cap(state.hidden_points, state.monthly_budget);
    if (clamped - state.hidden_points).abs() < f64::EPSILON {
        return Ok(state);
    }
    set_hidden_points(pool, &mut state, clamped).await?;
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            r#"
            CREATE TABLE reward_state (
                id TEXT PRIMARY KEY,
                version INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                deleted_at TEXT,
                hidden_points REAL NOT NULL,
                monthly_budget REAL NOT NULL
            );
            CREATE TABLE reward_drops (
                id TEXT PRIMARY KEY,
                version INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                deleted_at TEXT,
                reward_name TEXT NOT NULL,
                amount REAL NOT NULL
            );
            INSERT INTO reward_state (
                id, version, created_at, updated_at, deleted_at, hidden_points, monthly_budget
            ) VALUES (?, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', NULL, 0, 10000);
            "#,
        )
        .bind(REWARD_STATE_ID)
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn gacha_increases_points() {
        let pool = setup().await;
        let drop = run_gacha(&pool, 1.0).await.unwrap().expect("drop");
        assert_eq!(drop.reward_name, "gacha");
        assert!(drop.amount > 0.0);
        let state = get_state(&pool).await.unwrap();
        assert!((state.hidden_points - drop.amount).abs() < 1e-9);
    }

    #[tokio::test]
    async fn spend_and_reject_invalid_amount() {
        let pool = setup().await;
        run_gacha(&pool, 1.5).await.unwrap();
        let _ = spend_points(&pool, 500.0).await.unwrap();
        let err = spend_points(&pool, 123.0).await;
        assert!(matches!(err, Err(AppError::Validation(_))));
    }

    #[tokio::test]
    async fn soft_cap_clamps() {
        let pool = setup().await;
        sqlx::query("UPDATE reward_state SET hidden_points = 9000 WHERE id = ?")
            .bind(REWARD_STATE_ID)
            .execute(&pool)
            .await
            .unwrap();
        let state = apply_daily_soft_cap(&pool).await.unwrap();
        assert!((state.hidden_points - 8000.0).abs() < 1e-9);
    }
}
