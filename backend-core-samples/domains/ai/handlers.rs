//! Portfolio sample — domains/ai (Thin Axum routes for chat / logs.)
//! Source: src-tauri/src/domains/ai/handlers.rs

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

use super::models::{AiLog, ChatRequest, ChatResponse};
use super::services;
use crate::core::error::AppResult;
use crate::core::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ai/chat", post(chat))
        .route("/ai/logs", get(list_logs))
}

/// Direct UI chat against Gemini. Response + tokens go to `ai_logs`.
async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> AppResult<Json<ChatResponse>> {
    Ok(Json(
        services::chat(&state.pool, state.config.gemini_api_key.as_deref(), req).await?,
    ))
}

async fn list_logs(State(state): State<AppState>) -> AppResult<Json<Vec<AiLog>>> {
    Ok(Json(services::list_logs(&state.pool).await?))
}
