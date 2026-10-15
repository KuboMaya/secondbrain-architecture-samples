//! Portfolio sample — EventBus → SSE for live UI toasts.
//! Highlights: subscribe to the same bus, filter `RewardDropped`, KeepAlive ping.
//! Source: `domains/reward/handlers.rs` (`reward_stream`).

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::{Stream, StreamExt};
use serde_json::json;
use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};

use crate::core::event_bus::AppEvent;
use crate::core::AppState;

/// SSE: live `RewardDropped` events for FE toast / Resident widgets.
async fn reward_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|item| async move {
        match item {
            Ok(AppEvent::RewardDropped {
                task_id,
                drop_id,
                reward_name,
                amount,
            }) => {
                let data = json!({
                    "type": "reward_dropped",
                    "task_id": task_id,
                    "drop_id": drop_id,
                    "reward_name": reward_name,
                    "amount": amount,
                });
                Some(Ok(Event::default()
                    .event("reward_dropped")
                    .data(data.to_string())))
            }
            Ok(_) => None,
            Err(BroadcastStreamRecvError::Lagged(_)) => Some(Ok(Event::default()
                .event("lagged")
                .data("{\"type\":\"lagged\"}"))),
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(20))
            .text("ping"),
    )
}
