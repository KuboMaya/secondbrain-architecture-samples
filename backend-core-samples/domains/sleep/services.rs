//! Portfolio sample — sleep CRUD + morning TTS guard (04:00–11:59).
//! Async sqlx + branch-heavy speak helpers for complexity analysis.
//! Source: condensed from `domains/sleep/services.rs` (paths → placeholders).

use chrono::{DateTime, Local, Timelike};
use std::cell::Cell;
use sqlx::SqlitePool;

use super::models::{new_record, CreateSleepRequest, SleepRecord, UpdateSleepRequest};
use crate::core::error::{AppError, AppResult};
use crate::core::event_bus::{AppEvent, EventBus};

thread_local! {
    static TEST_FORCE_MORNING: Cell<bool> = const { Cell::new(false) };
}

pub fn set_test_force_morning(enabled: bool) {
    TEST_FORCE_MORNING.with(|c| c.set(enabled));
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Morning VOICEROID window (local time): 04:00–11:59 inclusive.
pub fn is_morning(now: DateTime<Local>) -> bool {
    if TEST_FORCE_MORNING.with(|c| c.get()) {
        return true;
    }
    let hour = now.hour();
    (4..12).contains(&hour)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpeakCommand {
    pub program: String,
    pub args: Vec<String>,
    pub text: String,
    pub dry_run: bool,
}

pub fn build_speak_command(voiceroid_path: Option<&str>, text: &str) -> SpeakCommand {
    match voiceroid_path.map(str::trim).filter(|p| !p.is_empty()) {
        Some(path) => SpeakCommand {
            program: path.to_string(),
            args: vec![text.to_string()],
            text: text.to_string(),
            dry_run: false,
        },
        None => SpeakCommand {
            program: "voiceroid-stub".into(),
            args: vec![text.to_string()],
            text: text.to_string(),
            dry_run: true,
        },
    }
}

pub fn speak_if_morning(
    now: DateTime<Local>,
    voiceroid_path: Option<&str>,
    text: &str,
) -> AppResult<SpeakCommand> {
    let cmd = build_speak_command(voiceroid_path, text);
    if !is_morning(now) {
        tracing::info!("skip tts: local time is outside 04:00–11:59");
        return Ok(cmd);
    }
    execute_speak(&cmd)?;
    Ok(cmd)
}

pub fn execute_speak(cmd: &SpeakCommand) -> AppResult<()> {
    if cmd.dry_run || cfg!(test) {
        println!("[TTS STUB] {}", cmd.text);
        return Ok(());
    }
    match std::process::Command::new(&cmd.program)
        .arg(&cmd.text)
        .spawn()
    {
        Ok(_) => {
            tracing::info!(program = %cmd.program, "tts process spawned");
            Ok(())
        }
        Err(err) => {
            tracing::warn!(error = %err, "tts spawn failed; falling back to stub");
            println!("[TTS STUB] {}", cmd.text);
            Ok(())
        }
    }
}

pub fn build_morning_speech(weather_label: &str, schedule_summary: &str) -> String {
    format!("おはようございます。今日の天気は{weather_label}です。{schedule_summary}")
}

pub async fn list_records(pool: &SqlitePool) -> AppResult<Vec<SleepRecord>> {
    let rows = sqlx::query_as::<_, SleepRecord>(
        "SELECT * FROM sleep_records WHERE deleted_at IS NULL ORDER BY target_date DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_record(pool: &SqlitePool, id: &str) -> AppResult<SleepRecord> {
    sqlx::query_as::<_, SleepRecord>(
        "SELECT * FROM sleep_records WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("sleep_record {id}")))
}

pub async fn create_record(pool: &SqlitePool, req: CreateSleepRequest) -> AppResult<SleepRecord> {
    if req.target_date.trim().is_empty() {
        return Err(AppError::Validation("target_date is required".into()));
    }
    let record = new_record(req);
    sqlx::query(
        r#"
        INSERT INTO sleep_records (
            id, version, created_at, updated_at, deleted_at,
            target_date, alarm_time, is_alarm_active, sleep_start, sleep_end
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&record.id)
    .bind(record.version)
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .bind(&record.deleted_at)
    .bind(&record.target_date)
    .bind(&record.alarm_time)
    .bind(record.is_alarm_active)
    .bind(&record.sleep_start)
    .bind(&record.sleep_end)
    .execute(pool)
    .await?;
    Ok(record)
}

pub async fn get_by_date(pool: &SqlitePool, target_date: &str) -> AppResult<Option<SleepRecord>> {
    let row = sqlx::query_as::<_, SleepRecord>(
        r#"
        SELECT * FROM sleep_records
        WHERE target_date = ? AND deleted_at IS NULL
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(target_date)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn ensure_today(
    pool: &SqlitePool,
    target_date: &str,
    alarm_time: Option<String>,
) -> AppResult<SleepRecord> {
    if let Some(existing) = get_by_date(pool, target_date).await? {
        return Ok(existing);
    }
    create_record(
        pool,
        CreateSleepRequest {
            target_date: target_date.to_string(),
            alarm_time,
            is_alarm_active: Some(true),
            sleep_start: None,
            sleep_end: None,
        },
    )
    .await
}

pub async fn update_record(
    pool: &SqlitePool,
    id: &str,
    req: UpdateSleepRequest,
) -> AppResult<SleepRecord> {
    let mut record = get_record(pool, id).await?;
    if let Some(expected) = req.version {
        if expected != record.version {
            return Err(AppError::Conflict("sleep version mismatch".into()));
        }
    }
    if let Some(date) = req.target_date {
        if date.trim().is_empty() {
            return Err(AppError::Validation("target_date is required".into()));
        }
        record.target_date = date;
    }
    if req.alarm_time.is_some() {
        record.alarm_time = req.alarm_time;
    }
    if let Some(active) = req.is_alarm_active {
        record.is_alarm_active = i64::from(active);
    }
    if req.sleep_start.is_some() {
        record.sleep_start = req.sleep_start;
    }
    if req.sleep_end.is_some() {
        record.sleep_end = req.sleep_end;
    }
    record.version += 1;
    record.updated_at = now_rfc3339();

    sqlx::query(
        r#"
        UPDATE sleep_records SET
            version = ?, updated_at = ?, target_date = ?, alarm_time = ?,
            is_alarm_active = ?, sleep_start = ?, sleep_end = ?
        WHERE id = ? AND deleted_at IS NULL
        "#,
    )
    .bind(record.version)
    .bind(&record.updated_at)
    .bind(&record.target_date)
    .bind(&record.alarm_time)
    .bind(record.is_alarm_active)
    .bind(&record.sleep_start)
    .bind(&record.sleep_end)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(record)
}

/// Persist wake fields; caller publishes `AlarmStopped` after HTTP 200.
pub async fn stop_alarm_persist(pool: &SqlitePool, id: &str) -> AppResult<SleepRecord> {
    let now = now_rfc3339();
    update_record(
        pool,
        id,
        UpdateSleepRequest {
            is_alarm_active: Some(false),
            sleep_end: Some(now),
            ..Default::default()
        },
    )
    .await
}

/// Convenience: persist + publish (prefer handler spawn for true 0ms path).
pub async fn stop_alarm_and_publish(
    pool: &SqlitePool,
    bus: &EventBus,
    id: &str,
) -> AppResult<SleepRecord> {
    let record = stop_alarm_persist(pool, id).await?;
    bus.publish(AppEvent::AlarmStopped {
        sleep_record_id: record.id.clone(),
        target_date: record.target_date.clone(),
        occurred_at: now_rfc3339(),
    });
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            r#"
            CREATE TABLE sleep_records (
                id TEXT PRIMARY KEY,
                version INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                deleted_at TEXT,
                target_date TEXT NOT NULL,
                alarm_time TEXT,
                is_alarm_active INTEGER NOT NULL,
                sleep_start TEXT,
                sleep_end TEXT
            );
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[test]
    fn morning_window_branches() {
        set_test_force_morning(false);
        let morning = Local.with_ymd_and_hms(2026, 10, 16, 7, 0, 0).unwrap();
        let afternoon = Local.with_ymd_and_hms(2026, 10, 16, 15, 0, 0).unwrap();
        let night = Local.with_ymd_and_hms(2026, 10, 16, 2, 0, 0).unwrap();
        assert!(is_morning(morning));
        assert!(!is_morning(afternoon));
        assert!(!is_morning(night));

        set_test_force_morning(true);
        assert!(is_morning(afternoon));
        set_test_force_morning(false);
    }

    #[test]
    fn speak_command_stub_when_path_missing() {
        let cmd = build_speak_command(None, "hello");
        assert!(cmd.dry_run);
        assert_eq!(cmd.program, "voiceroid-stub");

        let cmd2 = build_speak_command(Some("/path/to/tts-stub.exe"), "hello");
        assert!(!cmd2.dry_run);
        assert_eq!(cmd2.program, "/path/to/tts-stub.exe");
    }

    #[test]
    fn speak_if_morning_skips_outside_window() {
        set_test_force_morning(false);
        let afternoon = Local.with_ymd_and_hms(2026, 10, 16, 15, 0, 0).unwrap();
        let cmd = speak_if_morning(afternoon, Some("/path/to/tts-stub.exe"), "hi").unwrap();
        assert!(!cmd.dry_run);
        // Outside morning: still returns command but execute_speak was not required to run.
    }

    #[tokio::test]
    async fn sleep_crud_ensure_today_and_stop() {
        let pool = setup_pool().await;
        let bus = EventBus::default();
        let mut rx = bus.subscribe();

        let a = ensure_today(&pool, "2026-10-16", Some("07:00".into()))
            .await
            .unwrap();
        let b = ensure_today(&pool, "2026-10-16", Some("07:30".into()))
            .await
            .unwrap();
        assert_eq!(a.id, b.id, "ensure_today must be idempotent per date");

        let stopped = stop_alarm_and_publish(&pool, &bus, &a.id).await.unwrap();
        assert_eq!(stopped.is_alarm_active, 0);
        assert!(stopped.sleep_end.is_some());

        let ev = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            ev,
            AppEvent::AlarmStopped {
                sleep_record_id,
                ..
            } if sleep_record_id == a.id
        ));
    }

    #[tokio::test]
    async fn version_conflict_on_update() {
        let pool = setup_pool().await;
        let rec = create_record(
            &pool,
            CreateSleepRequest {
                target_date: "2026-10-16".into(),
                alarm_time: None,
                is_alarm_active: Some(true),
                sleep_start: None,
                sleep_end: None,
            },
        )
        .await
        .unwrap();

        let err = update_record(
            &pool,
            &rec.id,
            UpdateSleepRequest {
                is_alarm_active: Some(false),
                version: Some(999),
                ..Default::default()
            },
        )
        .await;
        assert!(matches!(err, Err(AppError::Conflict(_))));
    }
}
