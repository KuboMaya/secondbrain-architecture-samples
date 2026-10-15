//! Portfolio sample — 「0ms 応答」パターン（タスク完了）。
//! Highlights: own-domain DB write → EventBus publish → immediate HTTP 200.
//! Cross-domain work (reward gacha, AI) runs only on bus subscribers.
//! Source: combined excerpt from `task/handlers.rs` + `task/services.rs`.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use sqlx::SqlitePool;

use crate::core::event_bus::{AppEvent, EventBus};
use crate::core::error::AppResult;
use crate::core::AppState;
use crate::domains::task::models::{AiAccepted, Task, UpdateTaskRequest};

// ── Handler (HTTP edge stays thin) ──────────────────────────────────────────

/// Own-domain write + publish, then return. Reward gacha is background.
async fn complete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Task>> {
    Ok(Json(
        complete_task_service(&state.pool, &state.event_bus, &id).await?,
    ))
}

/// Publish AI request on the bus, return 200 before Gemini finishes.
async fn suggest_priority(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<(StatusCode, Json<AiAccepted>)> {
    let task = get_task(&state.pool, &id).await?;
    let request_id =
        request_priority_suggestion(&state.event_bus, &task.id, &task.title);
    Ok((
        StatusCode::OK,
        Json(AiAccepted {
            accepted: true,
            request_id,
        }),
    ))
}

// ── Service (persist, then Pub — caller returns immediately) ────────────────

/// Completes the task, persists, then publishes `TaskCompleted`.
/// Callers return HTTP 200 right after this; gacha is a subscriber.
pub async fn complete_task_service(
    pool: &SqlitePool,
    bus: &EventBus,
    id: &str,
) -> AppResult<Task> {
    let task = get_task(pool, id).await?;
    if task.status == "done" {
        return Ok(task);
    }

    let updated = update_task(
        pool,
        id,
        UpdateTaskRequest {
            status: Some("done".into()),
            version: Some(task.version),
            ..Default::default()
        },
    )
    .await?;

    bus.publish(AppEvent::TaskCompleted {
        task_id: updated.id.clone(),
        multiplier: updated.reward_multiplier,
    });
    Ok(updated)
}

// Production helpers live in the same domain module (not shown).
use crate::domains::task::ai_handler::request_priority_suggestion;
use crate::domains::task::services::{get_task, update_task};
