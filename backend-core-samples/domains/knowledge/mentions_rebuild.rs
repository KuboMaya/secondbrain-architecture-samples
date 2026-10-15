//! Portfolio sample — bidirectional dictionary Mentions (backlinks).
//! On save: rebuild edges where this entry is source or target via keyword⊂content.
//! Source: excerpt from `domains/knowledge/services.rs`.

use sqlx::SqlitePool;

use crate::core::common_models::{now_rfc3339, BaseFields};
use crate::core::error::AppResult;

async fn insert_mention(
    pool: &SqlitePool,
    source_entry_id: &str,
    target_entry_id: &str,
) -> AppResult<()> {
    if source_entry_id == target_entry_id {
        return Ok(());
    }
    let id = BaseFields::new().id;
    let created_at = now_rfc3339();
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO dictionary_mentions
            (id, source_entry_id, target_entry_id, created_at)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(source_entry_id)
    .bind(target_entry_id)
    .bind(&created_at)
    .execute(pool)
    .await?;
    Ok(())
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// Rebuild all mention edges that involve `entry_id` as source or target.
///
/// 1. As target: other entries whose `content` contains this keyword
/// 2. As source: other keywords that appear in this `content`
pub async fn rebuild_mentions_for_entry(pool: &SqlitePool, entry_id: &str) -> AppResult<()> {
    let entry = get_entry(pool, entry_id).await?;

    sqlx::query(
        "DELETE FROM dictionary_mentions WHERE source_entry_id = ? OR target_entry_id = ?",
    )
    .bind(entry_id)
    .bind(entry_id)
    .execute(pool)
    .await?;

    let keyword = entry.keyword.trim();
    if !keyword.is_empty() {
        let pattern = format!("%{}%", escape_like(keyword));
        let source_ids: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT id FROM dictionary
            WHERE deleted_at IS NULL
              AND id != ?
              AND content LIKE ? ESCAPE '\'
            "#,
        )
        .bind(entry_id)
        .bind(&pattern)
        .fetch_all(pool)
        .await?;
        for source_id in source_ids {
            insert_mention(pool, &source_id, entry_id).await?;
        }
    }

    #[derive(sqlx::FromRow)]
    struct KeywordRow {
        id: String,
        keyword: String,
    }

    let others: Vec<KeywordRow> = sqlx::query_as(
        r#"
        SELECT id, keyword FROM dictionary
        WHERE deleted_at IS NULL AND id != ?
        "#,
    )
    .bind(entry_id)
    .fetch_all(pool)
    .await?;

    for other in others {
        let other_kw = other.keyword.trim();
        if other_kw.is_empty() {
            continue;
        }
        if entry.content.contains(other_kw) {
            insert_mention(pool, entry_id, &other.id).await?;
        }
    }

    Ok(())
}

// Production: `get_entry` lives in the same services module.
use crate::domains::knowledge::services::get_entry;
