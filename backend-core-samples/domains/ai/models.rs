//! Portfolio sample — domains/ai (Request/response + AiLog DTOs.)
//! Source: src-tauri/src/domains/ai/models.rs

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::core::common_models::{now_rfc3339, BaseFields};

#[derive(Clone, Debug, Serialize, Deserialize, FromRow)]
pub struct AiLog {
    pub id: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub event_type: String,
    pub prompt: String,
    pub response: String,
    pub token_usage: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatResponse {
    pub request_id: String,
    pub response: String,
    pub token_usage: i64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenerateRequest {
    pub event_type: String,
    pub prompt: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct GenerateResponse {
    pub request_id: String,
    pub response: String,
    pub token_usage: i64,
}

pub fn new_log(event_type: String, prompt: String, response: String, token_usage: i64) -> AiLog {
    let base = BaseFields::new();
    AiLog {
        id: base.id,
        version: base.version,
        created_at: base.created_at.clone(),
        updated_at: now_rfc3339(),
        deleted_at: None,
        event_type,
        prompt,
        response,
        token_usage,
    }
}
