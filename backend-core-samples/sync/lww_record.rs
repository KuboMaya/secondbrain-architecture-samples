//! Portfolio sample — SyncRecord wire format + Last-Write-Wins.
//! Highlights: version is the sole ordering key; equal → keep local.
//! Source: `src-tauri/src/sync/record.rs` (comments reframed for Tailscale peers).

use serde::{Deserialize, Serialize};

/// Canonical sync unit exchanged between PC (source of truth) and PWA peers.
///
/// `version` is a monotonic counter for Last-Write-Wins against the local row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRecord {
    pub id: String,
    /// Logical domain / table namespace (e.g. `"tasks"`, `"schedules"`).
    pub domain: String,
    pub version: i64,
    pub is_deleted: bool,
    /// AES-GCM ciphertext of the domain payload (Base64), or empty for tombstones.
    pub encrypted_data: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LwwDecision {
    /// Remote version is strictly newer — apply remote.
    AcceptRemote,
    /// Local version is newer or equal — keep local.
    KeepLocal,
}

/// LWW: accept remote only when `remote.version > local_version`.
/// A missing local row is treated by the caller as insert (`AcceptRemote`).
pub fn lww_decide(local_version: Option<i64>, remote_version: i64) -> LwwDecision {
    match local_version {
        None => LwwDecision::AcceptRemote,
        Some(local) if remote_version > local => LwwDecision::AcceptRemote,
        Some(_) => LwwDecision::KeepLocal,
    }
}

impl SyncRecord {
    pub fn tombstone(id: impl Into<String>, domain: impl Into<String>, version: i64) -> Self {
        Self {
            id: id.into(),
            domain: domain.into(),
            version,
            is_deleted: true,
            encrypted_data: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lww_accepts_newer_remote() {
        assert_eq!(lww_decide(Some(1), 2), LwwDecision::AcceptRemote);
        assert_eq!(lww_decide(None, 1), LwwDecision::AcceptRemote);
        assert_eq!(lww_decide(Some(5), 5), LwwDecision::KeepLocal);
        assert_eq!(lww_decide(Some(10), 3), LwwDecision::KeepLocal);
    }
}
