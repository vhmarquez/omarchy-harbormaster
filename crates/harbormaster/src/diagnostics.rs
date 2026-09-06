//! Pure projection of already-owned snapshots, with no I/O or mutation handles.
use crate::{
    protocol::Revision,
    recovery::RecoveryStatus,
    storage::{MaintenanceResult, StoragePolicy, StorageStatus},
};

/// A fixed-schema JSON projection, even with maximum-width numeric fields.
pub const MAX_DIAGNOSTIC_BYTES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticError {
    MismatchedRevision,
}
impl std::fmt::Display for DiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("mismatched_snapshot_revision")
    }
}
impl std::error::Error for DiagnosticError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Scope {
    ManagerMetadata,
}

/// This value has only closed codes and numeric/boolean observations. It cannot
/// contain an identifier, path, source string or environment entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DiagnosticSnapshot {
    scope: Scope,
    revision: Revision,
    history_days: i64,
    storage: StorageCounts,
    cleanup: CleanupCounts,
    recovery: RecoveryStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct StorageCounts {
    facts: u32,
    tombstones: u32,
    attention: u32,
    pending_deliveries: u32,
    discarded_events: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct CleanupCounts {
    facts_removed: u32,
    tombstones_removed: u32,
    generations_retired: u32,
    deliveries_expired: u32,
    more: bool,
}
impl From<MaintenanceResult> for CleanupCounts {
    fn from(result: MaintenanceResult) -> Self {
        Self {
            facts_removed: result.facts_removed,
            tombstones_removed: result.tombstones_removed,
            generations_retired: result.generations_retired,
            deliveries_expired: result.deliveries_expired,
            more: result.more,
        }
    }
}

/// Project already-owned immutable snapshots; no collection, policy or storage
/// action is performed. Recovery counters are independent volatile observations,
/// not an assertion of a cross-filesystem atomic storage revision.
/// # Errors
/// Reject storage status and policy that describe different committed revisions.
pub fn project(
    status: &StorageStatus,
    policy: &StoragePolicy,
    recovery: &RecoveryStatus,
) -> Result<DiagnosticSnapshot, DiagnosticError> {
    if status.revision != policy.revision {
        return Err(DiagnosticError::MismatchedRevision);
    }
    Ok(DiagnosticSnapshot {
        scope: Scope::ManagerMetadata,
        revision: status.revision,
        history_days: policy.history.days(),
        storage: StorageCounts {
            facts: status.facts,
            tombstones: status.tombstones,
            attention: status.attention,
            pending_deliveries: status.pending_deliveries,
            discarded_events: status.discarded_events,
        },
        cleanup: status.cleanup.into(),
        recovery: *recovery,
    })
}
