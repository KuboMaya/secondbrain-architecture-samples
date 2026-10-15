//! Portfolio sample — alarm stop with 0ms HTTP path.
//! Highlights: persist wake → return JSON → `tokio::spawn` publishes `AlarmStopped`.
//! Morning briefing (weather / schedules / Gemini / TTS) is entirely background.
//! Source: `sleep/handlers.rs` (`stop_alarm`).

use axum::extract::{Path, State};
use axum::Json;

use crate::core::common_models::now_utc;
use crate::core::event_bus::AppEvent;
use crate::core::error::AppResult;
use crate::core::AppState;
use crate::domains::sleep::models::SleepRecord;
use crate::domains::sleep::services;

/// Persist wake, return HTTP 200, then Pub `AlarmStopped` in the background.
async fn stop_alarm(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<SleepRecord>> {
    let record = services::stop_alarm_persist(&state.pool, &id).await?;
    let bus = state.event_bus.clone();
    let event = AppEvent::AlarmStopped {
        sleep_record_id: record.id.clone(),
        target_date: record.target_date.clone(),
        occurred_at: now_utc().to_rfc3339(),
    };
    // Do not await morning pipeline here — keep the HTTP path fast.
    tokio::spawn(async move {
        bus.publish(event);
    });
    Ok(Json(record))
}
