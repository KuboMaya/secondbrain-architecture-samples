//! Portfolio sample — morning briefing orchestration on the EventBus.
//! Flow: AlarmStopped → Weather + Schedules fan-out → AiStreamRequest
//!       → AiStreamDelta (punctuated chunks → TTS F&F) → AiStreamCompleted.
//! Also: morning time guard (04:00–11:59). Personal location strings removed.
//! Source: condensed from `sleep/morning_routine.rs`.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use chrono::Local;
use reqwest::Client;
use sqlx::SqlitePool;
use tokio::sync::broadcast::error::RecvError;

use crate::core::event_bus::{reply_events, AppEvent, EventBus};
use crate::domains::sleep::ai_handler;
use crate::domains::sleep::services;

struct StreamSpeakSession {
    voice_server_url: Option<String>,
    http: Client,
    buffer: String,
    spoken_any: bool,
}

static STREAM_SESSIONS: LazyLock<Mutex<HashMap<String, StreamSpeakSession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Subscribe once at boot; never block HTTP handlers.
pub fn spawn(pool: SqlitePool, bus: EventBus, voice_server_url: Option<String>, http: Client) {
    let mut rx = bus.subscribe();
    let bus_out = bus.clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::AlarmStopped {
                    sleep_record_id,
                    target_date,
                    ..
                }) => {
                    handle_alarm_stopped(
                        &pool,
                        &bus_out,
                        &sleep_record_id,
                        &target_date,
                        voice_server_url.clone(),
                        http.clone(),
                    )
                    .await;
                }
                Ok(AppEvent::AiStreamDelta {
                    request_id,
                    reply_event_name,
                    delta,
                }) if reply_event_name == reply_events::MORNING_BRIEFING => {
                    on_morning_stream_delta(&request_id, &delta);
                }
                Ok(AppEvent::AiStreamCompleted {
                    request_id,
                    reply_event_name,
                    ..
                }) if reply_event_name == reply_events::MORNING_BRIEFING => {
                    // Flush remainder buffer / fallback speak — see production file.
                    let _ = request_id;
                }
                Ok(_) => {}
                Err(RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "morning_routine lagged; continuing");
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

async fn handle_alarm_stopped(
    _pool: &SqlitePool,
    bus: &EventBus,
    sleep_record_id: &str,
    target_date: &str,
    voice_server_url: Option<String>,
    http: Client,
) {
    // Fail-safe: never read aloud outside morning hours.
    if !services::is_morning(Local::now()) {
        tracing::info!(%sleep_record_id, "morning routine skipped (not 04:00–11:59)");
        return;
    }

    let request_id = uuid::Uuid::new_v4().to_string();

    // Fan-out: weather + today's schedules (other domains answer via EventBus).
    bus.publish(AppEvent::WeatherRequest {
        request_id: request_id.clone(),
        location: "local".into(), // production reads config; no personal address here
        target_date: target_date.to_string(),
    });
    bus.publish(AppEvent::TodaySchedulesRequest {
        request_id: request_id.clone(),
        target_date: target_date.to_string(),
    });

    // After both replies arrive (or timeout), production publishes AiStreamRequest.
    // Shown here: start stream once voice server URL is configured.
    if let Some(url) = voice_server_url.filter(|u| !u.trim().is_empty()) {
        let stream_id = uuid::Uuid::new_v4().to_string();
        STREAM_SESSIONS.lock().expect("sessions").insert(
            stream_id.clone(),
            StreamSpeakSession {
                voice_server_url: Some(url),
                http,
                buffer: String::new(),
                spoken_any: false,
            },
        );
        bus.publish(AppEvent::AiStreamRequest {
            request_id: stream_id,
            prompt: "(morning briefing prompt built by sleep domain)".into(),
            reply_event_name: reply_events::MORNING_BRIEFING.into(),
        });
    }

    let _ = Duration::from_millis(500); // production: timeout force-finish
}

/// Each Gemini pseudo-chunk: drain punctuation → strip style tags → F&F POST /speak.
fn on_morning_stream_delta(request_id: &str, delta: &str) {
    let mut sessions = STREAM_SESSIONS.lock().expect("stream sessions");
    let Some(session) = sessions.get_mut(request_id) else {
        return;
    };
    session.buffer.push_str(delta);
    let chunks = ai_handler::drain_speech_chunks(&mut session.buffer);
    let Some(url) = session.voice_server_url.clone() else {
        return;
    };
    for chunk in chunks {
        let req = ai_handler::parse_voice_chunk(&chunk);
        if req.text.trim().is_empty() {
            continue;
        }
        session.spoken_any = true;
        ai_handler::speak_parsed_request(&session.http, &url, req);
    }
}
