//! Portfolio sample — SleepRecord DTOs.
//! Source: `domains/sleep/models.rs`.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct SleepRecord {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub target_date: String,
    pub alarm_time: Option<String>,
    pub is_alarm_active: i64,
    pub sleep_start: Option<String>,
    pub sleep_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSleepRequest {
    pub target_date: String,
    pub alarm_time: Option<String>,
    pub is_alarm_active: Option<bool>,
    pub sleep_start: Option<String>,
    pub sleep_end: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSleepRequest {
    pub target_date: Option<String>,
    pub alarm_time: Option<String>,
    pub is_alarm_active: Option<bool>,
    pub sleep_start: Option<String>,
    pub sleep_end: Option<String>,
    pub version: Option<i64>,
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn new_record(req: CreateSleepRequest) -> SleepRecord {
    let now = now_rfc3339();
    SleepRecord {
        id: Uuid::new_v4().to_string(),
        version: 1,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
        target_date: req.target_date,
        alarm_time: req.alarm_time,
        is_alarm_active: i64::from(req.is_alarm_active.unwrap_or(false)),
        sleep_start: req.sleep_start,
        sleep_end: req.sleep_end,
    }
}
