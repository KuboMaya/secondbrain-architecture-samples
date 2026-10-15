//! Portfolio sample — sync decision helpers (Insert / Update / Noop / Conflict).
//! Architecture v10 prefers `evaluate_sync_record` (version-only LWW).
//! Source: excerpt from `src-tauri/src/sync/engine.rs`.

use serde_json::Value;

use crate::core::error::{AppError, AppResult};
use crate::sync::record::{lww_decide, LwwDecision, SyncRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRecord {
    pub version: i64,
    pub updated_at: String,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingRecord {
    pub version: i64,
    pub updated_at: String,
    pub data: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncAction {
    Insert,
    Update,
    Noop,
}

/// Legacy path: version + updated_at tie-break (still covered by conflict tests).
pub fn evaluate_sync(
    local: Option<&LocalRecord>,
    incoming: &IncomingRecord,
) -> AppResult<SyncAction> {
    match local {
        None => Ok(SyncAction::Insert),
        Some(local) if incoming.version > local.version => Ok(SyncAction::Update),
        Some(local) if incoming.version < local.version => Err(AppError::SyncConflict {
            message: "stale cloud version".into(),
            local: local.data.clone(),
            cloud: incoming.data.clone(),
        }),
        Some(local) if incoming.updated_at == local.updated_at => Ok(SyncAction::Noop),
        Some(local) => Err(AppError::SyncConflict {
            message: "version conflict: same version, different updated_at".into(),
            local: local.data.clone(),
            cloud: incoming.data.clone(),
        }),
    }
}

/// Architecture v10 LWW path: `SyncRecord.version` is the sole ordering key.
pub fn evaluate_sync_record(local_version: Option<i64>, remote: &SyncRecord) -> LwwDecision {
    lww_decide(local_version, remote.version)
}
