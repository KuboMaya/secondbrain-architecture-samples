//! Portfolio sample — domains/ai (Generate + persist token usage to ai_logs.)
//! Source: src-tauri/src/domains/ai/services.rs

use sqlx::SqlitePool;

use super::client;
use super::models::{new_log, AiLog, ChatRequest, ChatResponse, GenerateRequest, GenerateResponse};
use crate::core::error::{AppError, AppResult};

/// UI chat: Gemini + `ai_logs` persistence.
pub async fn chat(
    pool: &SqlitePool,
    api_key: Option<&str>,
    req: ChatRequest,
) -> AppResult<ChatResponse> {
    if req.message.trim().is_empty() {
        return Err(AppError::Validation("message is required".into()));
    }
    let result = client::generate_content(api_key, &req.message).await;
    let log = new_log(
        "chat".into(),
        req.message,
        result.text.clone(),
        result.token_usage,
    );
    insert_log(pool, &log).await?;
    Ok(ChatResponse {
        request_id: log.id,
        response: result.text,
        token_usage: result.token_usage,
    })
}

/// Internal / EventBus inference path with explicit event_type for logging.
pub async fn generate(
    pool: &SqlitePool,
    api_key: Option<&str>,
    req: GenerateRequest,
) -> AppResult<GenerateResponse> {
    if req.prompt.trim().is_empty() {
        return Err(AppError::Validation("prompt is required".into()));
    }
    let event_type = if req.event_type.trim().is_empty() {
        "inference".to_string()
    } else {
        req.event_type
    };

    let result = client::generate_content(api_key, &req.prompt).await;
    let log = new_log(
        event_type,
        req.prompt,
        result.text.clone(),
        result.token_usage,
    );
    insert_log(pool, &log).await?;

    Ok(GenerateResponse {
        request_id: log.id,
        response: result.text,
        token_usage: result.token_usage,
    })
}

/// Streamed generate: logs once after completion; `on_delta` receives pseudo-chunks.
pub async fn generate_stream<F>(
    pool: &SqlitePool,
    api_key: Option<&str>,
    req: GenerateRequest,
    on_delta: F,
) -> AppResult<GenerateResponse>
where
    F: FnMut(&str),
{
    if req.prompt.trim().is_empty() {
        return Err(AppError::Validation("prompt is required".into()));
    }
    let event_type = if req.event_type.trim().is_empty() {
        "stream".to_string()
    } else {
        req.event_type
    };

    let result = client::generate_content_stream(api_key, &req.prompt, on_delta).await;
    let log = new_log(
        event_type,
        req.prompt,
        result.text.clone(),
        result.token_usage,
    );
    insert_log(pool, &log).await?;

    Ok(GenerateResponse {
        request_id: log.id,
        response: result.text,
        token_usage: result.token_usage,
    })
}

async fn insert_log(pool: &SqlitePool, log: &AiLog) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO ai_logs (
            id, version, created_at, updated_at, deleted_at,
            event_type, prompt, response, token_usage
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&log.id)
    .bind(log.version)
    .bind(&log.created_at)
    .bind(&log.updated_at)
    .bind(&log.deleted_at)
    .bind(&log.event_type)
    .bind(&log.prompt)
    .bind(&log.response)
    .bind(log.token_usage)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_logs(pool: &SqlitePool) -> AppResult<Vec<AiLog>> {
    let rows = sqlx::query_as::<_, AiLog>(
        "SELECT * FROM ai_logs WHERE deleted_at IS NULL ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
