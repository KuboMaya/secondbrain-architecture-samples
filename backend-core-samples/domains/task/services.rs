//! Portfolio sample — Task / Schedule async CRUD (sqlx) + EventBus complete.
//! Includes optimistic version checks, soft-delete, date filters, unit tests.
//! Source: `domains/task/services.rs` (+ portfolio-local test harness).

use sqlx::SqlitePool;

use super::models::{
    encode_tags, new_task, now_rfc3339, CreateScheduleRequest, CreateTaskRequest, Schedule, Task,
    UpdateScheduleRequest, UpdateTaskRequest,
};
use crate::core::error::{AppError, AppResult};
use crate::core::event_bus::{AppEvent, EventBus};

pub async fn list_tasks(pool: &SqlitePool) -> AppResult<Vec<Task>> {
    let rows = sqlx::query_as::<_, Task>(
        "SELECT * FROM tasks WHERE deleted_at IS NULL ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_task(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = ? AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task {id}")))
}

pub async fn create_task(pool: &SqlitePool, req: CreateTaskRequest) -> AppResult<Task> {
    if req.title.trim().is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    let task = new_task(req);
    sqlx::query(
        r#"
        INSERT INTO tasks (
            id, version, created_at, updated_at, deleted_at,
            title, status, priority, due_date, parent_id, tags, reward_multiplier
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&task.id)
    .bind(task.version)
    .bind(&task.created_at)
    .bind(&task.updated_at)
    .bind(&task.deleted_at)
    .bind(&task.title)
    .bind(&task.status)
    .bind(task.priority)
    .bind(&task.due_date)
    .bind(&task.parent_id)
    .bind(&task.tags)
    .bind(task.reward_multiplier)
    .execute(pool)
    .await?;
    Ok(task)
}

pub async fn update_task(pool: &SqlitePool, id: &str, req: UpdateTaskRequest) -> AppResult<Task> {
    let mut task = get_task(pool, id).await?;
    if let Some(expected) = req.version {
        if expected != task.version {
            return Err(AppError::Conflict(format!(
                "task {id} version mismatch (expected {}, actual {})",
                expected, task.version
            )));
        }
    }
    if let Some(title) = req.title {
        task.title = title;
    }
    if let Some(status) = req.status {
        task.status = status;
    }
    if let Some(priority) = req.priority {
        task.priority = priority;
    }
    if req.due_date.is_some() {
        task.due_date = req.due_date;
    }
    if req.parent_id.is_some() {
        task.parent_id = req.parent_id;
    }
    if let Some(tags) = req.tags {
        task.tags = encode_tags(&tags);
    }
    if let Some(multiplier) = req.reward_multiplier {
        task.reward_multiplier = multiplier;
    }
    task.version += 1;
    task.updated_at = now_rfc3339();

    sqlx::query(
        r#"
        UPDATE tasks SET
            version = ?, updated_at = ?, title = ?, status = ?, priority = ?,
            due_date = ?, parent_id = ?, tags = ?, reward_multiplier = ?
        WHERE id = ? AND deleted_at IS NULL
        "#,
    )
    .bind(task.version)
    .bind(&task.updated_at)
    .bind(&task.title)
    .bind(&task.status)
    .bind(task.priority)
    .bind(&task.due_date)
    .bind(&task.parent_id)
    .bind(&task.tags)
    .bind(task.reward_multiplier)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(task)
}

pub async fn delete_task(pool: &SqlitePool, id: &str) -> AppResult<Task> {
    let mut task = get_task(pool, id).await?;
    task.deleted_at = Some(now_rfc3339());
    task.version += 1;
    task.updated_at = now_rfc3339();
    sqlx::query("UPDATE tasks SET deleted_at = ?, version = ?, updated_at = ? WHERE id = ?")
        .bind(&task.deleted_at)
        .bind(task.version)
        .bind(&task.updated_at)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(task)
}

/// Persist done + publish. Callers return HTTP 200 immediately; gacha is a subscriber.
pub async fn complete_task(pool: &SqlitePool, bus: &EventBus, id: &str) -> AppResult<Task> {
    let task = get_task(pool, id).await?;
    if task.status == "done" {
        return Ok(task);
    }

    let updated = update_task(
        pool,
        id,
        UpdateTaskRequest {
            status: Some("done".into()),
            version: Some(task.version),
            ..Default::default()
        },
    )
    .await?;

    bus.publish(AppEvent::TaskCompleted {
        task_id: updated.id.clone(),
        multiplier: updated.reward_multiplier,
    });
    Ok(updated)
}

pub async fn list_schedules(pool: &SqlitePool) -> AppResult<Vec<Schedule>> {
    let rows = sqlx::query_as::<_, Schedule>(
        "SELECT * FROM schedules WHERE deleted_at IS NULL ORDER BY start_time ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_schedules_for_date(
    pool: &SqlitePool,
    target_date: &str,
) -> AppResult<Vec<Schedule>> {
    let pattern = format!("{target_date}%");
    let rows = sqlx::query_as::<_, Schedule>(
        r#"
        SELECT * FROM schedules
        WHERE deleted_at IS NULL AND start_time LIKE ?
        ORDER BY start_time ASC
        "#,
    )
    .bind(&pattern)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_tasks_for_date(pool: &SqlitePool, target_date: &str) -> AppResult<Vec<Task>> {
    let rows = sqlx::query_as::<_, Task>(
        r#"
        SELECT * FROM tasks
        WHERE deleted_at IS NULL
          AND status != 'done'
          AND (due_date IS NULL OR due_date = ? OR due_date LIKE ?)
        ORDER BY priority DESC, created_at ASC
        "#,
    )
    .bind(target_date)
    .bind(format!("{target_date}%"))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub fn summarize_today(
    target_date: &str,
    schedules: &[Schedule],
    tasks: &[Task],
) -> (String, Vec<String>, Vec<String>) {
    let schedule_titles: Vec<String> = schedules.iter().map(|s| s.title.clone()).collect();
    let task_titles: Vec<String> = tasks.iter().map(|t| t.title.clone()).collect();

    let schedule_part = if schedule_titles.is_empty() {
        "本日の予定はありません。".to_string()
    } else {
        format!("本日の予定は{}です。", schedule_titles.join("、"))
    };
    let task_part = if task_titles.is_empty() {
        "残タスクはありません。".to_string()
    } else {
        format!("残っているタスクは{}です。", task_titles.join("、"))
    };
    let summary = format!("{target_date}。{schedule_part}{task_part}");
    (summary, schedule_titles, task_titles)
}

pub async fn get_schedule(pool: &SqlitePool, id: &str) -> AppResult<Schedule> {
    sqlx::query_as::<_, Schedule>("SELECT * FROM schedules WHERE id = ? AND deleted_at IS NULL")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("schedule {id}")))
}

pub async fn create_schedule(pool: &SqlitePool, req: CreateScheduleRequest) -> AppResult<Schedule> {
    if req.title.trim().is_empty() {
        return Err(AppError::Validation("title is required".into()));
    }
    let now = now_rfc3339();
    let schedule = Schedule {
        id: uuid::Uuid::new_v4().to_string(),
        version: 1,
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
        title: req.title,
        start_time: req.start_time,
        end_time: req.end_time,
        is_all_day: i64::from(req.is_all_day.unwrap_or(false)),
        recurrence_rule: req.recurrence_rule,
        location: req.location,
        notes: req.notes,
    };
    sqlx::query(
        r#"
        INSERT INTO schedules (
            id, version, created_at, updated_at, deleted_at,
            title, start_time, end_time, is_all_day, recurrence_rule, location, notes
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&schedule.id)
    .bind(schedule.version)
    .bind(&schedule.created_at)
    .bind(&schedule.updated_at)
    .bind(&schedule.deleted_at)
    .bind(&schedule.title)
    .bind(&schedule.start_time)
    .bind(&schedule.end_time)
    .bind(schedule.is_all_day)
    .bind(&schedule.recurrence_rule)
    .bind(&schedule.location)
    .bind(&schedule.notes)
    .execute(pool)
    .await?;
    Ok(schedule)
}

pub async fn update_schedule(
    pool: &SqlitePool,
    id: &str,
    req: UpdateScheduleRequest,
) -> AppResult<Schedule> {
    let mut schedule = get_schedule(pool, id).await?;
    if let Some(expected) = req.version {
        if expected != schedule.version {
            return Err(AppError::Conflict("schedule version mismatch".into()));
        }
    }
    if let Some(title) = req.title {
        schedule.title = title;
    }
    if let Some(start_time) = req.start_time {
        schedule.start_time = start_time;
    }
    if let Some(end_time) = req.end_time {
        schedule.end_time = end_time;
    }
    if let Some(is_all_day) = req.is_all_day {
        schedule.is_all_day = i64::from(is_all_day);
    }
    if req.recurrence_rule.is_some() {
        schedule.recurrence_rule = req.recurrence_rule;
    }
    if req.location.is_some() {
        schedule.location = req.location;
    }
    if req.notes.is_some() {
        schedule.notes = req.notes;
    }
    schedule.version += 1;
    schedule.updated_at = now_rfc3339();
    sqlx::query(
        r#"
        UPDATE schedules SET
            version = ?, updated_at = ?, title = ?, start_time = ?, end_time = ?,
            is_all_day = ?, recurrence_rule = ?, location = ?, notes = ?
        WHERE id = ? AND deleted_at IS NULL
        "#,
    )
    .bind(schedule.version)
    .bind(&schedule.updated_at)
    .bind(&schedule.title)
    .bind(&schedule.start_time)
    .bind(&schedule.end_time)
    .bind(schedule.is_all_day)
    .bind(&schedule.recurrence_rule)
    .bind(&schedule.location)
    .bind(&schedule.notes)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(schedule)
}

pub async fn delete_schedule(pool: &SqlitePool, id: &str) -> AppResult<Schedule> {
    let mut schedule = get_schedule(pool, id).await?;
    schedule.deleted_at = Some(now_rfc3339());
    schedule.version += 1;
    schedule.updated_at = now_rfc3339();
    sqlx::query("UPDATE schedules SET deleted_at = ?, version = ?, updated_at = ? WHERE id = ?")
        .bind(&schedule.deleted_at)
        .bind(schedule.version)
        .bind(&schedule.updated_at)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(schedule)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            r#"
            CREATE TABLE tasks (
                id TEXT PRIMARY KEY,
                version INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                deleted_at TEXT,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                priority INTEGER NOT NULL,
                due_date TEXT,
                parent_id TEXT,
                tags TEXT NOT NULL,
                reward_multiplier REAL NOT NULL
            );
            CREATE TABLE schedules (
                id TEXT PRIMARY KEY,
                version INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                deleted_at TEXT,
                title TEXT NOT NULL,
                start_time TEXT NOT NULL,
                end_time TEXT NOT NULL,
                is_all_day INTEGER NOT NULL,
                recurrence_rule TEXT,
                location TEXT,
                notes TEXT
            );
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn create_list_get_update_delete_task() {
        let pool = setup_pool().await;
        let created = create_task(
            &pool,
            CreateTaskRequest {
                title: "sample task".into(),
                priority: Some(2),
                tags: Some(vec!["demo".into()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(created.status, "todo");
        assert_eq!(created.version, 1);

        let listed = list_tasks(&pool).await.unwrap();
        assert_eq!(listed.len(), 1);

        let updated = update_task(
            &pool,
            &created.id,
            UpdateTaskRequest {
                title: Some("renamed".into()),
                version: Some(1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.title, "renamed");
        assert_eq!(updated.version, 2);

        let conflict = update_task(
            &pool,
            &created.id,
            UpdateTaskRequest {
                title: Some("stale".into()),
                version: Some(1),
                ..Default::default()
            },
        )
        .await;
        assert!(matches!(conflict, Err(AppError::Conflict(_))));

        let deleted = delete_task(&pool, &created.id).await.unwrap();
        assert!(deleted.deleted_at.is_some());
        assert!(get_task(&pool, &created.id).await.is_err());
    }

    #[tokio::test]
    async fn complete_task_publishes_once() {
        let pool = setup_pool().await;
        let bus = EventBus::default();
        let mut rx = bus.subscribe();

        let task = create_task(
            &pool,
            CreateTaskRequest {
                title: "finish me".into(),
                reward_multiplier: Some(1.5),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let done = complete_task(&pool, &bus, &task.id).await.unwrap();
        assert_eq!(done.status, "done");

        let event = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
            .expect("event")
            .expect("open");
        match event {
            AppEvent::TaskCompleted {
                task_id,
                multiplier,
            } => {
                assert_eq!(task_id, task.id);
                assert!((multiplier - 1.5).abs() < f64::EPSILON);
            }
            other => panic!("unexpected {other:?}"),
        }

        complete_task(&pool, &bus, &task.id).await.unwrap();
        let second = tokio::time::timeout(std::time::Duration::from_millis(80), rx.recv()).await;
        assert!(second.is_err(), "idempotent complete must not republish");
    }

    #[tokio::test]
    async fn schedule_crud_and_date_filter() {
        let pool = setup_pool().await;
        let s = create_schedule(
            &pool,
            CreateScheduleRequest {
                title: "standup".into(),
                start_time: "2026-10-16T09:00:00+09:00".into(),
                end_time: "2026-10-16T09:15:00+09:00".into(),
                is_all_day: Some(false),
                recurrence_rule: None,
                location: Some("http://100.x.x.x:3000/meet".into()),
                notes: None,
            },
        )
        .await
        .unwrap();

        let day = list_schedules_for_date(&pool, "2026-10-16").await.unwrap();
        assert_eq!(day.len(), 1);
        assert_eq!(day[0].id, s.id);

        let empty = list_schedules_for_date(&pool, "2026-10-17").await.unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn summarize_today_branches() {
        let (summary, _, _) = summarize_today("2026-10-16", &[], &[]);
        assert!(summary.contains("予定はありません"));
        assert!(summary.contains("残タスクはありません"));
    }

    #[tokio::test]
    async fn validation_rejects_empty_title() {
        let pool = setup_pool().await;
        let err = create_task(
            &pool,
            CreateTaskRequest {
                title: "   ".into(),
                ..Default::default()
            },
        )
        .await;
        assert!(matches!(err, Err(AppError::Validation(_))));
    }
}
