//! Portfolio sample — domain-driven AI (knowledge owns prompts/parsing).
//! Never calls Gemini directly: only Pub `AiInferenceRequest` and await reply on the bus.
//! Source: excerpt from `domains/knowledge/ai_handler.rs`.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;

use crate::core::common_models::{strip_code_fence, BaseFields};
use crate::core::error::{AppError, AppResult};
use crate::core::event_bus::{reply_events, AppEvent, EventBus};

static PENDING_FROM_AI: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

const TRANSLATE_TIMEOUT: Duration = Duration::from_secs(45);

/// Build registration prompt, Pub request, return `request_id` (HTTP can 200 immediately).
pub fn begin_register_from_ai(bus: &EventBus, ai_response_text: &str) -> String {
    let request_id = BaseFields::new().id;
    {
        let mut guard = PENDING_FROM_AI.lock().expect("pending map");
        guard.insert(request_id.clone(), ai_response_text.to_string());
    }

    let prompt = format!(
        "以下の文章を個人用辞書に登録する。JSONのみで答えよ:\n\
{{\"keyword\":\"題名を10文字以内\",\"tags\":[\"関連タグ\"]}}\n\
文章:\n{ai_response_text}"
    );
    bus.publish(AppEvent::AiInferenceRequest {
        request_id: request_id.clone(),
        prompt,
        reply_event_name: reply_events::KNOWLEDGE_AI_COMPLETED.into(),
    });
    request_id
}

pub fn parse_translation(response: &str) -> String {
    strip_code_fence(response)
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_string()
}

/// Pub translate request and wait for the matching `AiInferenceCompleted` (with timeout).
pub async fn request_translation(bus: &EventBus, japanese: &str) -> AppResult<String> {
    let request_id = BaseFields::new().id;
    let prompt = format!(
        "指定されたテキストを英語学習用に自然な英語へ。本文のみ出力。\n\n{japanese}"
    );
    let mut rx = bus.subscribe();
    bus.publish(AppEvent::AiInferenceRequest {
        request_id: request_id.clone(),
        prompt,
        reply_event_name: reply_events::KNOWLEDGE_TRANSLATE.into(),
    });

    let wait = async {
        loop {
            match rx.recv().await {
                Ok(AppEvent::AiInferenceCompleted {
                    request_id: rid,
                    reply_event_name,
                    response,
                    ..
                }) if rid == request_id
                    && reply_event_name == reply_events::KNOWLEDGE_TRANSLATE =>
                {
                    return Ok(parse_translation(&response));
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue;
                Err(RecvError::Closed) => {
                    return Err(AppError::Internal("event bus closed during translate".into()));
                }
            }
        }
    };

    match tokio::time::timeout(TRANSLATE_TIMEOUT, wait).await {
        Ok(result) => result,
        Err(_) => Err(AppError::Internal("translation timed out".into())),
    }
}
