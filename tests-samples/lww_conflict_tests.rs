//! Portfolio sample — LWW / sync conflict boundary tests (production-style).
//! Mirrors `src-tauri/tests/sync/conflict_tests.rs` (unit layer; HTTP 409 omitted).
//! Source: conflict matrix for version / updated_at.

use second_brain::core::error::AppError;
use second_brain::sync::engine::{evaluate_sync, IncomingRecord, LocalRecord, SyncAction};
use second_brain::sync::record::{lww_decide, LwwDecision};
use serde_json::json;

fn local(version: i64, updated_at: &str, title: &str) -> LocalRecord {
    LocalRecord {
        version,
        updated_at: updated_at.into(),
        data: json!({
            "id": "local",
            "version": version,
            "updated_at": updated_at,
            "payload": { "title": title }
        }),
    }
}

fn incoming(version: i64, updated_at: &str, title: &str) -> IncomingRecord {
    IncomingRecord {
        version,
        updated_at: updated_at.into(),
        data: json!({
            "id": "peer",
            "version": version,
            "updated_at": updated_at,
            "payload": { "title": title }
        }),
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
    match evaluate_sync(Some(&local), &incoming(2, "2026-01-02T00:00:00Z", "peer")).unwrap_err() {
        AppError::SyncConflict {
            message,
            local: l,
            cloud: c,
        } => {
            assert!(message.contains("stale"));
            assert_eq!(l["payload"]["title"], "local");
            assert_eq!(c["payload"]["title"], "peer");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn noop_when_same_version_and_same_updated_at() {
    let ts = "2026-01-02T00:00:00Z";
    assert_eq!(
        evaluate_sync(Some(&local(2, ts, "same")), &incoming(2, ts, "same")).unwrap(),
        SyncAction::Noop
    );
}

/// Boundary: versions equal, updated_at differs by 1 second → conflict with both sides.
#[test]
fn conflict_boundary_same_version_different_updated_at() {
    let local = local(5, "2026-09-30T10:00:00Z", "mine");
    let peer = incoming(5, "2026-09-30T10:00:01Z", "theirs");
    match evaluate_sync(Some(&local), &peer).unwrap_err() {
        AppError::SyncConflict {
            message,
            local: l,
            cloud: c,
        } => {
            assert!(message.contains("updated_at"));
            assert_eq!(l["version"], 5);
            assert_eq!(c["version"], 5);
            assert_ne!(l["updated_at"], c["updated_at"]);
            assert_eq!(l["payload"]["title"], "mine");
            assert_eq!(c["payload"]["title"], "theirs");
        }
        other => panic!("unexpected {other:?}"),
    }
}

/// Boundary: version equal, updated_at identical → Noop (even if payload text differs).
#[test]
fn boundary_same_version_same_updated_at_is_noop_not_conflict() {
    let ts = "2026-09-30T10:00:00.000Z";
    assert_eq!(
        evaluate_sync(Some(&local(1, ts, "x")), &incoming(1, ts, "y")).unwrap(),
        SyncAction::Noop
    );
}

/// Boundary: version just above local (n → n+1) → Update.
#[test]
fn boundary_version_plus_one_updates() {
    let local = local(10, "2026-01-01T00:00:00Z", "a");
    assert_eq!(
        evaluate_sync(Some(&local), &incoming(11, "2026-01-01T00:00:01Z", "b")).unwrap(),
        SyncAction::Update
    );
}

#[test]
fn lww_decide_matrix() {
    assert_eq!(lww_decide(None, 1), LwwDecision::AcceptRemote);
    assert_eq!(lww_decide(Some(1), 2), LwwDecision::AcceptRemote);
    assert_eq!(lww_decide(Some(5), 5), LwwDecision::KeepLocal);
    assert_eq!(lww_decide(Some(10), 3), LwwDecision::KeepLocal);
}

#[test]
fn lww_decide_large_gap_still_accepts_newer() {
    assert_eq!(lww_decide(Some(1), 1_000_000), LwwDecision::AcceptRemote);
    assert_eq!(lww_decide(Some(1_000_000), 999_999), LwwDecision::KeepLocal);
}
