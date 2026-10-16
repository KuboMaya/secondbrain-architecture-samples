//! Portfolio sample — domain-driven AI for task priority (EventBus only).
//! HTTP returns immediately after `request_priority_suggestion`; apply happens in listener.
//! Source: `domains/task/ai_handler.rs`.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use sqlx::SqlitePool;
use uuid::Uuid;

use super::models::UpdateTaskRequest;
use super::services;
use crate::core::event_bus::{reply_events, AppEvent, EventBus};

static PENDING: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Fire-and-forget AI priority suggestion (0ms path for HTTP callers).
pub fn request_priority_suggestion(bus: &EventBus, task_id: &str, task_title: &str) -> String {
    let request_id = Uuid::new_v4().to_string();
    PENDING
        .lock()
        .expect("pending")
        .insert(request_id.clone(), task_id.to_string());
    let prompt = format!(
        "Suggest a priority integer 0-3 and a one-line rationale for this task as JSON \
         with keys priority and rationale.\nTask: {task_title}"
    );
    bus.publish(AppEvent::AiInferenceRequest {
        request_id: request_id.clone(),
        prompt,
        reply_event_name: reply_events::TASK_AI_REPLY.into(),
    });
    request_id
}

pub fn handle_ai_reply(response: &str) -> Option<serde_json::Value> {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(response) {
        return Some(v);
    }
    // Fallback: extract first `{...}` object from noisy model output.
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            if end > start {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&response[start..=end]) {
                    return Some(v);
                }
            }
        }
    }
    Some(serde_json::json!({ "raw": response, "parsed": false }))
}

fn extract_priority(parsed: &serde_json::Value) -> Option<i64> {
    parsed
        .get("priority")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .filter(|p| (0..=3).contains(p))
}

pub fn spawn_listener(pool: SqlitePool, bus: EventBus) {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::AiInferenceCompleted {
                    reply_event_name,
                    response,
                    token_usage,
                    request_id,
                }) if reply_event_name == reply_events::TASK_AI_REPLY => {
                    let task_id = PENDING.lock().expect("pending").remove(&request_id);
                    let parsed = handle_ai_reply(&response);
                    tracing::info!(%request_id, token_usage, parsed = ?parsed, "task ai reply");
                    let Some(task_id) = task_id else { continue };
                    let Some(parsed) = parsed else { continue };
                    let Some(priority) = extract_priority(&parsed) else { continue };
                    match services::update_task(
                        &pool,
                        &task_id,
                        UpdateTaskRequest {
                            priority: Some(priority),
                            ..Default::default()
                        },
                    )
                    .await
                    {
                        Ok(_) => tracing::info!(%task_id, priority, "applied AI priority"),
                        Err(err) => tracing::warn!(%task_id, error = %err, "apply failed"),
                    }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "task ai_handler lagged; continuing");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_and_noisy_wrapper() {
        let v = handle_ai_reply(r#"{"priority":2,"rationale":"important"}"#).unwrap();
        assert_eq!(extract_priority(&v), Some(2));

        let v2 = handle_ai_reply("Sure.\n{\"priority\":3,\"rationale\":\"x\"}\n").unwrap();
        assert_eq!(extract_priority(&v2), Some(3));

        assert_eq!(
            extract_priority(&serde_json::json!({"priority": 9})),
            None
        );
    }
}
