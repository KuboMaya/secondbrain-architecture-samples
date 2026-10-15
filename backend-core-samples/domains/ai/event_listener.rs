//! Portfolio sample — domains/ai (Subscribes to AiInference/AiStream; publishes replies on the bus.)
//! Source: src-tauri/src/domains/ai/event_listener.rs

use sqlx::SqlitePool;
use tokio::sync::broadcast::error::RecvError;

use super::models::GenerateRequest;
use super::services;
use crate::core::event_bus::{reply_events, AppEvent, EventBus};

/// Listen for `AiInferenceRequest` / `AiStreamRequest`, run Gemini + log, then Pub replies.
pub fn spawn(pool: SqlitePool, bus: EventBus, gemini_api_key: Option<String>) {
    let mut rx = bus.subscribe();
    let bus_out = bus.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::AiInferenceRequest {
                    request_id,
                    prompt,
                    reply_event_name,
                }) => {
                    let result = services::generate(
                        &pool,
                        gemini_api_key.as_deref(),
                        GenerateRequest {
                            event_type: reply_event_name.clone(),
                            prompt,
                        },
                    )
                    .await;

                    let (response, token_usage) = match result {
                        Ok(resp) => (resp.response, resp.token_usage),
                        Err(err) => {
                            tracing::error!(
                                request_id = %request_id,
                                error = %err,
                                "ai inference failed"
                            );
                            (format!(r#"{{"error":"{err}"}}"#), 0)
                        }
                    };

                    publish_reply(
                        &bus_out,
                        &reply_event_name,
                        request_id,
                        response,
                        token_usage,
                    );
                }
                Ok(AppEvent::AiStreamRequest {
                    request_id,
                    prompt,
                    reply_event_name,
                }) => {
                    let bus_delta = bus_out.clone();
                    let rid = request_id.clone();
                    let reply = reply_event_name.clone();
                    let result = services::generate_stream(
                        &pool,
                        gemini_api_key.as_deref(),
                        GenerateRequest {
                            event_type: reply_event_name.clone(),
                            prompt,
                        },
                        |delta| {
                            bus_delta.publish(AppEvent::AiStreamDelta {
                                request_id: rid.clone(),
                                reply_event_name: reply.clone(),
                                delta: delta.to_string(),
                            });
                        },
                    )
                    .await;

                    let (full_text, token_usage) = match result {
                        Ok(resp) => (resp.response, resp.token_usage),
                        Err(err) => {
                            tracing::error!(
                                request_id = %request_id,
                                error = %err,
                                "ai stream failed"
                            );
                            (String::new(), 0)
                        }
                    };

                    bus_out.publish(AppEvent::AiStreamCompleted {
                        request_id,
                        reply_event_name,
                        full_text,
                        token_usage,
                    });
                }
                Ok(_) => {}
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "ai listener lagged; continuing");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

fn publish_reply(
    bus: &EventBus,
    reply_event_name: &str,
    request_id: String,
    response: String,
    token_usage: i64,
) {
    if reply_event_name == reply_events::KNOWLEDGE_AI_COMPLETED {
        bus.publish(AppEvent::KnowledgeAiCompleted {
            request_id,
            response,
            token_usage,
        });
    } else {
        bus.publish(AppEvent::AiInferenceCompleted {
            request_id,
            reply_event_name: reply_event_name.to_string(),
            response,
            token_usage,
        });
    }
}
