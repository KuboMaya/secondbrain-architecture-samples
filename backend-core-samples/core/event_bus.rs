//! Portfolio sample — EventBus (domain Pub/Sub).
//! Highlights: typed AppEvent, tokio::sync::broadcast, no cross-domain imports.
//! Source: src-tauri/src/core/event_bus.rs (logic unchanged).

use tokio::sync::broadcast;

/// Well-known `reply_event_name` values used by domain `ai_handler`s.
pub mod reply_events {
    pub const TASK_AI_REPLY: &str = "task.ai.reply";
    pub const KNOWLEDGE_AI_COMPLETED: &str = "KnowledgeAiCompleted";
    /// English-learning translation for a dictionary entry.
    pub const KNOWLEDGE_TRANSLATE: &str = "KnowledgeTranslate";
    /// Auto-categorize tags + index suggestion.
    pub const KNOWLEDGE_CATEGORIZE: &str = "KnowledgeCategorize";
    /// Async dictionary ai_summary (background; length-gated).
    pub const KNOWLEDGE_SUMMARY: &str = "KnowledgeSummary";
    /// Morning briefing speech (streamed).
    pub const MORNING_BRIEFING: &str = "morning.briefing";
}

/// Type-safe application events.
/// Domains must not import each other; they communicate only through this bus.
#[derive(Clone, Debug, PartialEq)]
pub enum AppEvent {
    TaskCompleted {
        task_id: String,
        multiplier: f64,
    },
    /// Sleep wake / alarm stop (morning routine trigger).
    AlarmStopped {
        sleep_record_id: String,
        target_date: String,
        occurred_at: String,
    },
    /// Morning / callers → Weather domain.
    WeatherRequest {
        request_id: String,
        location: String,
        target_date: String,
    },
    WeatherResponse {
        request_id: String,
        summary_text: String,
        weather_label: String,
    },
    /// Morning → Task/Schedule domain.
    TodaySchedulesRequest {
        request_id: String,
        target_date: String,
    },
    TodaySchedulesResponse {
        request_id: String,
        summary_text: String,
        schedule_titles: Vec<String>,
        task_titles: Vec<String>,
    },
    AiInferenceRequest {
        request_id: String,
        prompt: String,
        reply_event_name: String,
    },
    AiInferenceCompleted {
        request_id: String,
        reply_event_name: String,
        response: String,
        token_usage: i64,
    },
    /// Streamed Gemini generation (pseudo-chunk or SSE). Prompt built by caller domain.
    AiStreamRequest {
        request_id: String,
        prompt: String,
        reply_event_name: String,
    },
    AiStreamDelta {
        request_id: String,
        reply_event_name: String,
        delta: String,
    },
    AiStreamCompleted {
        request_id: String,
        reply_event_name: String,
        full_text: String,
        token_usage: i64,
    },
    KnowledgeAiCompleted {
        request_id: String,
        response: String,
        token_usage: i64,
    },
    /// Reward domain finished a gacha roll (may be zero / skipped → not published).
    RewardDropped {
        task_id: String,
        drop_id: String,
        reward_name: String,
        amount: f64,
    },
    /// Device domain unwrapped the shared master key and injected it into sync.
    MasterKeyInstalled {
        device_id: String,
    },
    /// User tapped「タスクに追加」(or legacy draft accept) — task domain creates a formal task.
    ShoppingListAccepted {
        draft_id: String,
        title: String,
        inventory_item_id: String,
        related_items: Vec<String>,
        category: Option<String>,
    },
    /// Task domain finished creating a shopping task (inventory may link it).
    TaskCreatedFromShopping {
        draft_id: String,
        task_id: String,
    },
}

pub type DomainEvent = AppEvent;

#[derive(Clone, Debug)]
pub struct EventBus {
    sender: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, event: AppEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.sender.subscribe()
    }

    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}
