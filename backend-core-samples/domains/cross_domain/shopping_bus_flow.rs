//! Portfolio sample — cross-domain shopping without direct domain imports.
//!
//! ```text
//! inventory HTTP  --Pub-->  ShoppingListAccepted
//! task listener   --creates task-->  TaskCreatedFromShopping
//! inventory listener  --links draft.task_id-->
//! ```
//!
//! Source: inventory/handlers.rs + task/event_listener.rs + inventory/event_listener.rs

use sqlx::SqlitePool;
use tokio::sync::broadcast::error::RecvError;

use crate::core::event_bus::{AppEvent, EventBus};
use crate::core::AppState;
use crate::domains::inventory::models::ShoppingDraft;
use crate::domains::inventory::services as inventory;
use crate::domains::task::models::CreateTaskRequest;
use crate::domains::task::services as task;

// ── 1) inventory HTTP: write draft, Pub, return 200 ─────────────────────────

fn publish_shopping_accepted(state: &AppState, draft: &ShoppingDraft) {
    let related: Vec<String> =
        serde_json::from_str(&draft.related_items).unwrap_or_default();
    state.event_bus.publish(AppEvent::ShoppingListAccepted {
        draft_id: draft.id.clone(),
        title: draft.suggested_title.clone(),
        inventory_item_id: draft.inventory_item_id.clone(),
        related_items: related,
        category: draft.suggested_category.clone(),
    });
}

// ── 2) task subscriber: create formal task, Pub link event ──────────────────

pub async fn handle_shopping_accepted(
    pool: &SqlitePool,
    bus: &EventBus,
    draft_id: String,
    title: String,
    inventory_item_id: String,
    related_items: Vec<String>,
    category: Option<String>,
) {
    let mut tags = vec!["買い物".into(), "inventory".into()];
    if let Some(cat) = category.filter(|s| !s.trim().is_empty()) {
        tags.push(cat);
    }
    for rel in related_items.into_iter().take(5) {
        let t = rel.trim();
        if !t.is_empty() && !tags.iter().any(|x| x == t) {
            tags.push(t.to_string());
        }
    }
    tags.push(format!("inv:{inventory_item_id}"));

    if let Ok(created) = task::create_task(
        pool,
        CreateTaskRequest {
            title,
            status: Some("todo".into()),
            priority: Some(2),
            due_date: None,
            parent_id: None,
            tags: Some(tags),
            reward_multiplier: Some(1.0),
        },
    )
    .await
    {
        bus.publish(AppEvent::TaskCreatedFromShopping {
            draft_id,
            task_id: created.id,
        });
    }
}

// ── 3) inventory subscriber: close the loop ─────────────────────────────────

pub fn spawn_inventory_linker(pool: SqlitePool, bus: EventBus) {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(AppEvent::TaskCreatedFromShopping { draft_id, task_id }) => {
                    let _ = inventory::link_shopping_draft_task(&pool, &draft_id, &task_id).await;
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
}

#[allow(dead_code)]
fn _keep_publish_visible() {
    let _ = publish_shopping_accepted as fn(&AppState, &ShoppingDraft);
}
