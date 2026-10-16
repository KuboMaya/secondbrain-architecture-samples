//! Portfolio sample — concrete EventBus subscriber runtime (spawn / lag / closed).
//! Complements `event_bus.rs`: typed Pub + async Sub with non-fatal lag handling.
//! Patterns taken from `reward/event_listener.rs` and `ai/event_listener.rs`.

use sqlx::SqlitePool;
use tokio::sync::broadcast::error::RecvError;

use super::event_bus::{AppEvent, EventBus};

/// Background worker: TaskCompleted → side-effect → optional RewardDropped republish.
/// HTTP handlers never await this; they only `publish` then return 200.
pub fn spawn_reward_style_listener<F, Fut>(pool: SqlitePool, bus: EventBus, on_completed: F)
where
    F: Fn(SqlitePool, String, f64) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<Option<(String, String, f64)>, String>> + Send,
{
    let mut rx = bus.subscribe();
    let bus_out = bus.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::TaskCompleted {
                    task_id,
                    multiplier,
                }) => match on_completed(pool.clone(), task_id.clone(), multiplier).await {
                    Ok(Some((drop_id, reward_name, amount))) => {
                        bus_out.publish(AppEvent::RewardDropped {
                            task_id,
                            drop_id,
                            reward_name,
                            amount,
                        });
                    }
                    Ok(None) => {
                        tracing::info!(%task_id, multiplier, "side-effect skipped");
                    }
                    Err(err) => {
                        tracing::error!(%task_id, %err, "side-effect failed; continuing");
                    }
                },
                Ok(_) => {}
                Err(RecvError::Lagged(skipped)) => {
                    // Slow consumer: drop oldest, keep listening (never panic).
                    tracing::warn!(skipped, "event listener lagged; continuing");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

/// AI-style listener: inference request → async work → typed reply on the same bus.
pub fn spawn_ai_style_listener<F, Fut>(bus: EventBus, handle: F)
where
    F: Fn(String, String) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = (String, i64)> + Send,
{
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
                    let (response, token_usage) = handle(request_id.clone(), prompt).await;
                    bus_out.publish(AppEvent::AiInferenceCompleted {
                        request_id,
                        reply_event_name,
                        response,
                        token_usage,
                    });
                }
                Ok(AppEvent::AiStreamRequest {
                    request_id,
                    prompt,
                    reply_event_name,
                }) => {
                    // Pseudo-stream: emit deltas then completed (mirrors domains/ai).
                    let (full_text, token_usage) = handle(request_id.clone(), prompt).await;
                    for chunk in punctuate_chunks(&full_text) {
                        bus_out.publish(AppEvent::AiStreamDelta {
                            request_id: request_id.clone(),
                            reply_event_name: reply_event_name.clone(),
                            delta: chunk,
                        });
                    }
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

fn punctuate_chunks(text: &str) -> Vec<String> {
    const PUNCT: &[char] = &['。', '、', '！', '？', '!', '?', '\n'];
    let mut out = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        buf.push(ch);
        if PUNCT.contains(&ch) {
            let piece = std::mem::take(&mut buf);
            if !piece.trim().is_empty() {
                out.push(piece);
            }
        }
    }
    if !buf.trim().is_empty() {
        out.push(buf);
    }
    if out.is_empty() && !text.is_empty() {
        out.push(text.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn lagged_subscriber_does_not_abort_bus() {
        let bus = EventBus::new(2);
        let mut slow = bus.subscribe();
        // Fill channel so a slow recv sees Lagged.
        for i in 0..8 {
            bus.publish(AppEvent::TaskCompleted {
                task_id: format!("t{i}"),
                multiplier: 1.0,
            });
        }
        // First recv may be Lagged or an event depending on timing; must not panic.
        let _ = slow.recv().await;
        bus.publish(AppEvent::TaskCompleted {
            task_id: "final".into(),
            multiplier: 1.0,
        });
        assert!(bus.receiver_count() >= 1);
    }

    #[tokio::test]
    async fn reward_style_listener_republishes_drop() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        let seen = Arc::new(Mutex::new(None));
        let seen2 = seen.clone();

        spawn_reward_style_listener(SqlitePool::connect_lazy("sqlite::memory:").unwrap(), bus.clone(), {
            let seen2 = seen2.clone();
            move |_pool, task_id, _mult| {
                let seen2 = seen2.clone();
                async move {
                    *seen2.lock().await = Some(task_id.clone());
                    Ok(Some(("drop-1".into(), "gacha".into(), 10.0)))
                }
            }
        });

        bus.publish(AppEvent::TaskCompleted {
            task_id: "task-a".into(),
            multiplier: 1.0,
        });

        let mut got_drop = false;
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            while let Ok(ev) = rx.try_recv() {
                if matches!(ev, AppEvent::RewardDropped { .. }) {
                    got_drop = true;
                }
            }
            if got_drop {
                break;
            }
        }
        assert!(got_drop, "RewardDropped should be published");
        assert_eq!(seen.lock().await.as_deref(), Some("task-a"));
    }

    #[test]
    fn punctuate_splits_japanese() {
        let parts = punctuate_chunks("こんにちは。今日は晴れです");
        assert!(parts.len() >= 2);
        assert_eq!(parts.concat(), "こんにちは。今日は晴れです");
    }
}
