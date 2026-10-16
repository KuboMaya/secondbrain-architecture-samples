//! Portfolio sample — Task / Schedule DTOs (sqlx FromRow).
//! Source: `domains/task/models.rs`.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct Task {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub title: String,
    pub status: String,
    pub priority: i64,
    pub due_date: Option<String>,
    pub parent_id: Option<String>,
    pub tags: String,
    pub reward_multiplier: f64,
}

impl Task {
    pub fn tag_list(&self) -> Vec<String> {
        serde_json::from_str(&self.tags).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateTaskRequest {
    pub title: String,
    pub status: Option<String>,
    pub priority: Option<i64>,
    pub due_date: Option<String>,
    pub parent_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub reward_multiplier: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i64>,
    pub due_date: Option<String>,
    pub parent_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub reward_multiplier: Option<f64>,
    pub version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, PartialEq)]
pub struct Schedule {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub title: String,
    pub start_time: String,
    pub end_time: String,
    pub is_all_day: i64,
    pub recurrence_rule: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScheduleRequest {
    pub title: String,
    pub start_time: String,
    pub end_time: String,
    pub is_all_day: Option<bool>,
    pub recurrence_rule: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateScheduleRequest {
    pub title: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub is_all_day: Option<bool>,
    pub recurrence_rule: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub version: Option<i64>,
}

pub fn encode_tags(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn new_task(req: CreateTaskRequest) -> Task {
    let now = now_rfc3339();
    Task {
        id: Uuid::new_v4().to_string(),
        version: 1,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
        title: req.title,
        status: req.status.unwrap_or_else(|| "todo".to_string()),
        priority: req.priority.unwrap_or(0),
        due_date: req.due_date,
        parent_id: req.parent_id,
        tags: encode_tags(&req.tags.unwrap_or_default()),
        reward_multiplier: req.reward_multiplier.unwrap_or(1.0),
    }
}
