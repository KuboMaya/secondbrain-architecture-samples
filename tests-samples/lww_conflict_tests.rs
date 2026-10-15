//! Portfolio sample — LWW / sync conflict tests pin the contract.
//! Source: excerpt from `src-tauri/tests/sync/conflict_tests.rs`.

use second_brain::core::error::AppError;
use second_brain::sync::engine::{evaluate_sync, IncomingRecord, LocalRecord, SyncAction};
use second_brain::sync::record::{lww_decide, LwwDecision};
use serde_json::json;

fn local(version: i64, updated_at: &str, title: &str) -> LocalRecord {
    LocalRecord {
        version,
        updated_at: updated_at.into(),
        data: json!({ "payload": { "title": title } }),
    }
}

fn incoming(version: i64, updated_at: &str, title: &str) -> IncomingRecord {
    IncomingRecord {
        version,
        updated_at: updated_at.into(),
        data: json!({ "payload": { "title": title } }),
    }
}

#[test]
fn insert_when_local_missing() {
    assert_eq!(
        evaluate_sync(None, &incoming(1, "2026-01-01T00:00:00Z", "a")).unwrap(),
        SyncAction::Insert
    );
}

#[test]
fn update_when_remote_version_greater() {
    let local = local(1, "2026-01-01T00:00:00Z", "a");
    assert_eq!(
        evaluate_sync(Some(&local), &incoming(2, "2026-01-02T00:00:00Z", "b")).unwrap(),
        SyncAction::Update
    );
}

#[test]
fn conflict_when_remote_version_stale() {
    let local = local(3, "2026-01-03T00:00:00Z", "local");
    match evaluate_sync(Some(&local), &incoming(2, "2026-01-02T00:00:00Z", "cloud")).unwrap_err()
    {
        AppError::SyncConflict { message, .. } => assert!(message.contains("stale")),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn lww_decide_keeps_equal_or_older() {
    assert_eq!(lww_decide(Some(5), 5), LwwDecision::KeepLocal);
    assert_eq!(lww_decide(Some(10), 3), LwwDecision::KeepLocal);
    assert_eq!(lww_decide(Some(1), 2), LwwDecision::AcceptRemote);
}
